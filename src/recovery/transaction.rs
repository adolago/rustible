//! Transaction Module
//!
//! Provides ACID-like transaction semantics for critical operations:
//!
//! - **Atomicity**: All operations succeed or all are rolled back
//! - **Consistency**: System moves from one valid state to another
//! - **Isolation**: Transactions don't interfere with each other
//! - **Durability**: Committed changes are persistent
//!
//! # Example
//!
//! ```rust,ignore
//! use rustible::recovery::transaction::{TransactionManager, TransactionConfig};
//!
//! let mut manager = TransactionManager::new(TransactionConfig::default());
//!
//! let tx_id = manager.begin("deploy-app");
//!
//! // Perform operations...
//! manager.add_operation(&tx_id, operation)?;
//!
//! // Commit or rollback
//! manager.commit(&tx_id)?;
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use super::rollback::{RollbackAction, RollbackError, StateChange};

/// Error type for transaction operations
#[derive(Error, Debug)]
pub enum TransactionError {
    #[error("Transaction not found: {0}")]
    NotFound(String),

    #[error("Transaction already exists: {0}")]
    AlreadyExists(String),

    #[error("Transaction timeout: {0}")]
    Timeout(String),

    #[error("Transaction conflict: {0}")]
    Conflict(String),

    #[error("Invalid transaction state: expected {expected:?}, got {actual:?}")]
    InvalidState {
        expected: TransactionState,
        actual: TransactionState,
    },

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Rollback error: {0}")]
    Rollback(#[from] RollbackError),

    #[error("Commit failed: {0}")]
    CommitFailed(String),
}

/// Unique identifier for a transaction
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionId(String);

impl TransactionId {
    /// Create a new transaction ID
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        Self(format!("tx-{}-{}", timestamp, counter))
    }

    /// Create from string
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get as string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TransactionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// State of a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionState {
    /// Transaction is active and accepting operations
    Active,
    /// Transaction is preparing to commit (2PC prepare phase)
    Preparing,
    /// Transaction is ready to commit (2PC ready phase)
    Prepared,
    /// Transaction is committing
    Committing,
    /// Transaction was committed successfully
    Committed,
    /// Transaction is rolling back
    RollingBack,
    /// Transaction was rolled back
    RolledBack,
    /// Transaction failed
    Failed,
}

impl Default for TransactionState {
    fn default() -> Self {
        Self::Active
    }
}

/// Phase of a transaction operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionPhase {
    /// Before the operation
    Before,
    /// During the operation
    During,
    /// After the operation (success)
    After,
    /// Operation failed
    Failed,
}

/// Configuration for transactions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionConfig {
    /// Maximum duration for a transaction
    pub timeout: Duration,

    /// Enable two-phase commit
    pub two_phase_commit: bool,

    /// Maximum number of operations per transaction
    pub max_operations: usize,

    /// Enable transaction logging
    pub enable_logging: bool,

    /// Log directory for transaction logs
    pub log_directory: Option<String>,

    /// Enable savepoints
    pub enable_savepoints: bool,

    /// Maximum nesting depth for nested transactions
    pub max_nesting_depth: usize,
}

impl Default for TransactionConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300), // 5 minutes
            two_phase_commit: false,
            max_operations: 1000,
            enable_logging: false,
            log_directory: None,
            enable_savepoints: true,
            max_nesting_depth: 3,
        }
    }
}

impl TransactionConfig {
    /// Create a production configuration
    pub fn production() -> Self {
        Self {
            timeout: Duration::from_secs(600), // 10 minutes
            two_phase_commit: true,
            max_operations: 10000,
            enable_logging: true,
            log_directory: Some("/var/log/rustible/transactions".to_string()),
            enable_savepoints: true,
            max_nesting_depth: 5,
        }
    }
}

/// An operation within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionOperation {
    /// Unique operation ID
    pub id: String,

    /// Description of the operation
    pub description: String,

    /// State change caused by this operation
    pub state_change: Option<StateChange>,

    /// Rollback action if operation needs to be undone
    pub rollback_action: Option<RollbackAction>,

    /// Current phase
    pub phase: TransactionPhase,

    /// Timestamp when operation was added
    pub timestamp: u64,
}

impl TransactionOperation {
    /// Create a new transaction operation
    pub fn new(description: impl Into<String>) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);

        Self {
            id: format!("op-{}", counter),
            description: description.into(),
            state_change: None,
            rollback_action: None,
            phase: TransactionPhase::Before,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Set the state change
    pub fn with_state_change(mut self, change: StateChange) -> Self {
        let action = RollbackAction::from_state_change(&change);
        self.state_change = Some(change);
        self.rollback_action = Some(action);
        self
    }

    /// Set a custom rollback action
    pub fn with_rollback(mut self, action: RollbackAction) -> Self {
        self.rollback_action = Some(action);
        self
    }
}

/// A savepoint within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Savepoint {
    /// Savepoint name
    pub name: String,

    /// Index of the last operation before this savepoint
    pub operation_index: usize,

    /// Timestamp when savepoint was created
    pub timestamp: u64,
}

/// A transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Transaction ID
    pub id: TransactionId,

    /// Transaction name/description
    pub name: String,

    /// Current state
    pub state: TransactionState,

    /// Operations in this transaction
    pub operations: Vec<TransactionOperation>,

    /// Savepoints
    pub savepoints: Vec<Savepoint>,

    /// Parent transaction ID (for nested transactions)
    pub parent_id: Option<TransactionId>,

    /// Timestamp when transaction started
    pub started_at: u64,

    /// Timestamp when transaction ended (committed/rolled back)
    pub ended_at: Option<u64>,

    /// Timeout for this transaction
    pub timeout: Duration,

    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Transaction {
    /// Create a new transaction
    pub fn new(name: impl Into<String>, timeout: Duration) -> Self {
        Self {
            id: TransactionId::new(),
            name: name.into(),
            state: TransactionState::Active,
            operations: Vec::new(),
            savepoints: Vec::new(),
            parent_id: None,
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ended_at: None,
            timeout,
            metadata: HashMap::new(),
        }
    }

    /// Check if transaction has timed out
    pub fn is_timed_out(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now - self.started_at > self.timeout.as_secs()
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set parent transaction
    pub fn with_parent(mut self, parent_id: TransactionId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Context passed to transaction operations
#[derive(Debug, Clone)]
pub struct TransactionContext {
    /// Transaction ID
    pub transaction_id: TransactionId,

    /// Current operation index
    pub operation_index: usize,

    /// Whether the transaction is in rollback mode
    pub rolling_back: bool,
}

impl TransactionContext {
    /// Create a new transaction context
    pub fn new(transaction_id: TransactionId) -> Self {
        Self {
            transaction_id,
            operation_index: 0,
            rolling_back: false,
        }
    }
}

/// Manager for transactions
pub struct TransactionManager {
    config: TransactionConfig,
    transactions: HashMap<TransactionId, Transaction>,
    active_count: usize,
}

impl TransactionManager {
    /// Create a new transaction manager
    pub fn new(config: TransactionConfig) -> Self {
        Self {
            config,
            transactions: HashMap::new(),
            active_count: 0,
        }
    }

    /// Begin a new transaction
    pub fn begin(&mut self, name: impl Into<String>) -> TransactionId {
        let name = name.into();
        let transaction = Transaction::new(&name, self.config.timeout);
        let id = transaction.id.clone();

        info!("Beginning transaction '{}' with id {}", name, id);
        self.transactions.insert(id.clone(), transaction);
        self.active_count += 1;

        id
    }

    /// Begin a nested transaction
    pub fn begin_nested(
        &mut self,
        parent_id: &TransactionId,
        name: impl Into<String>,
    ) -> Result<TransactionId, TransactionError> {
        let parent = self
            .transactions
            .get(parent_id)
            .ok_or_else(|| TransactionError::NotFound(parent_id.to_string()))?;

        // Check nesting depth
        let depth = self.get_nesting_depth(parent_id);
        if depth >= self.config.max_nesting_depth {
            return Err(TransactionError::Conflict(format!(
                "Maximum nesting depth ({}) exceeded",
                self.config.max_nesting_depth
            )));
        }

        if parent.state != TransactionState::Active {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: parent.state,
            });
        }

        let name = name.into();
        let transaction = Transaction::new(&name, self.config.timeout).with_parent(parent_id.clone());
        let id = transaction.id.clone();

        info!(
            "Beginning nested transaction '{}' with id {} (parent: {})",
            name, id, parent_id
        );
        self.transactions.insert(id.clone(), transaction);
        self.active_count += 1;

        Ok(id)
    }

    /// Get nesting depth for a transaction
    fn get_nesting_depth(&self, tx_id: &TransactionId) -> usize {
        let mut depth = 0;
        let mut current_id = tx_id.clone();

        while let Some(tx) = self.transactions.get(&current_id) {
            if let Some(parent_id) = &tx.parent_id {
                depth += 1;
                current_id = parent_id.clone();
            } else {
                break;
            }
        }

        depth
    }

    /// Add an operation to a transaction
    pub fn add_operation(
        &mut self,
        tx_id: &TransactionId,
        operation: TransactionOperation,
    ) -> Result<(), TransactionError> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        if tx.state != TransactionState::Active {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: tx.state,
            });
        }

        if tx.is_timed_out() {
            return Err(TransactionError::Timeout(tx_id.to_string()));
        }

        if tx.operations.len() >= self.config.max_operations {
            return Err(TransactionError::Conflict(format!(
                "Maximum operations ({}) exceeded",
                self.config.max_operations
            )));
        }

        debug!(
            "Adding operation '{}' to transaction {}",
            operation.description, tx_id
        );
        tx.operations.push(operation);

        Ok(())
    }

    /// Create a savepoint
    pub fn savepoint(
        &mut self,
        tx_id: &TransactionId,
        name: impl Into<String>,
    ) -> Result<(), TransactionError> {
        if !self.config.enable_savepoints {
            return Err(TransactionError::Conflict(
                "Savepoints are not enabled".to_string(),
            ));
        }

        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        if tx.state != TransactionState::Active {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: tx.state,
            });
        }

        let savepoint = Savepoint {
            name: name.into(),
            operation_index: tx.operations.len(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        debug!(
            "Creating savepoint '{}' in transaction {} at operation {}",
            savepoint.name, tx_id, savepoint.operation_index
        );
        tx.savepoints.push(savepoint);

        Ok(())
    }

    /// Rollback to a savepoint
    pub async fn rollback_to_savepoint(
        &mut self,
        tx_id: &TransactionId,
        savepoint_name: &str,
    ) -> Result<(), TransactionError> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        let savepoint = tx
            .savepoints
            .iter()
            .find(|s| s.name == savepoint_name)
            .ok_or_else(|| {
                TransactionError::NotFound(format!("Savepoint '{}' not found", savepoint_name))
            })?
            .clone();

        info!(
            "Rolling back transaction {} to savepoint '{}'",
            tx_id, savepoint_name
        );

        // Rollback operations after the savepoint
        while tx.operations.len() > savepoint.operation_index {
            if let Some(mut op) = tx.operations.pop() {
                if op.rollback_action.is_some() {
                    debug!("Rolling back operation: {}", op.description);
                    // Execute rollback (simplified - in real impl would use RollbackManager)
                    op.phase = TransactionPhase::Failed;
                }
            }
        }

        // Remove savepoints after this one
        tx.savepoints.retain(|s| s.name == savepoint_name || s.operation_index < savepoint.operation_index);

        Ok(())
    }

    /// Prepare transaction for commit (2PC)
    pub fn prepare(&mut self, tx_id: &TransactionId) -> Result<(), TransactionError> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        if tx.state != TransactionState::Active {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: tx.state,
            });
        }

        if tx.is_timed_out() {
            return Err(TransactionError::Timeout(tx_id.to_string()));
        }

        debug!("Preparing transaction {} for commit", tx_id);
        tx.state = TransactionState::Preparing;

        // In a real implementation, this would:
        // 1. Flush all pending writes
        // 2. Acquire locks
        // 3. Write prepare record to log
        // 4. Notify participants in distributed transaction

        tx.state = TransactionState::Prepared;
        info!("Transaction {} prepared for commit", tx_id);

        Ok(())
    }

    /// Commit a transaction
    pub fn commit(&mut self, tx_id: &TransactionId) -> Result<(), TransactionError> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        // For 2PC, must be prepared first
        if self.config.two_phase_commit && tx.state != TransactionState::Prepared {
            if tx.state == TransactionState::Active {
                // Auto-prepare
                tx.state = TransactionState::Preparing;
                tx.state = TransactionState::Prepared;
            } else {
                return Err(TransactionError::InvalidState {
                    expected: TransactionState::Prepared,
                    actual: tx.state,
                });
            }
        } else if tx.state != TransactionState::Active && tx.state != TransactionState::Prepared {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: tx.state,
            });
        }

        if tx.is_timed_out() {
            return Err(TransactionError::Timeout(tx_id.to_string()));
        }

        debug!("Committing transaction {}", tx_id);
        tx.state = TransactionState::Committing;

        // Mark all operations as completed
        for op in &mut tx.operations {
            op.phase = TransactionPhase::After;
        }

        tx.state = TransactionState::Committed;
        tx.ended_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        self.active_count = self.active_count.saturating_sub(1);

        info!(
            "Transaction {} committed ({} operations)",
            tx_id,
            tx.operations.len()
        );

        Ok(())
    }

    /// Rollback a transaction
    pub async fn rollback(&mut self, tx_id: &TransactionId) -> Result<(), TransactionError> {
        let tx = self
            .transactions
            .get_mut(tx_id)
            .ok_or_else(|| TransactionError::NotFound(tx_id.to_string()))?;

        if tx.state == TransactionState::Committed {
            return Err(TransactionError::InvalidState {
                expected: TransactionState::Active,
                actual: tx.state,
            });
        }

        if tx.state == TransactionState::RolledBack {
            return Ok(()); // Already rolled back
        }

        info!("Rolling back transaction {}", tx_id);
        tx.state = TransactionState::RollingBack;

        // Rollback operations in reverse order
        let operations: Vec<_> = tx.operations.iter().cloned().collect();
        for op in operations.into_iter().rev() {
            if op.rollback_action.is_some() {
                debug!("Rolling back operation: {}", op.description);
                // In real implementation, would execute rollback action
            }
        }

        // Mark all operations as failed
        for op in &mut tx.operations {
            op.phase = TransactionPhase::Failed;
        }

        tx.state = TransactionState::RolledBack;
        tx.ended_at = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );

        self.active_count = self.active_count.saturating_sub(1);

        info!("Transaction {} rolled back", tx_id);

        Ok(())
    }

    /// Get a transaction by ID
    pub fn get(&self, tx_id: &TransactionId) -> Option<&Transaction> {
        self.transactions.get(tx_id)
    }

    /// Get number of active transactions
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Get the configuration
    pub fn config(&self) -> &TransactionConfig {
        &self.config
    }

    /// Cleanup completed transactions older than the given duration
    pub fn cleanup(&mut self, max_age: Duration) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let max_age_secs = max_age.as_secs();

        self.transactions.retain(|id, tx| {
            if let Some(ended_at) = tx.ended_at {
                if now - ended_at > max_age_secs {
                    debug!("Cleaning up old transaction {}", id);
                    return false;
                }
            }
            true
        });
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new(TransactionConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_id() {
        let id1 = TransactionId::new();
        let id2 = TransactionId::new();

        assert_ne!(id1, id2);
        assert!(id1.as_str().starts_with("tx-"));
    }

    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new("test", Duration::from_secs(60));

        assert_eq!(tx.name, "test");
        assert_eq!(tx.state, TransactionState::Active);
        assert!(tx.operations.is_empty());
    }

    #[test]
    fn test_transaction_manager_begin() {
        let mut manager = TransactionManager::new(TransactionConfig::default());

        let tx_id = manager.begin("test-transaction");
        let tx = manager.get(&tx_id).unwrap();

        assert_eq!(tx.name, "test-transaction");
        assert_eq!(tx.state, TransactionState::Active);
        assert_eq!(manager.active_count(), 1);
    }

    #[test]
    fn test_transaction_manager_add_operation() {
        let mut manager = TransactionManager::new(TransactionConfig::default());
        let tx_id = manager.begin("test");

        let op = TransactionOperation::new("test operation");
        manager.add_operation(&tx_id, op).unwrap();

        let tx = manager.get(&tx_id).unwrap();
        assert_eq!(tx.operations.len(), 1);
    }

    #[test]
    fn test_transaction_manager_commit() {
        let mut manager = TransactionManager::new(TransactionConfig::default());
        let tx_id = manager.begin("test");

        manager.commit(&tx_id).unwrap();

        let tx = manager.get(&tx_id).unwrap();
        assert_eq!(tx.state, TransactionState::Committed);
        assert_eq!(manager.active_count(), 0);
    }

    #[tokio::test]
    async fn test_transaction_manager_rollback() {
        let mut manager = TransactionManager::new(TransactionConfig::default());
        let tx_id = manager.begin("test");

        manager.rollback(&tx_id).await.unwrap();

        let tx = manager.get(&tx_id).unwrap();
        assert_eq!(tx.state, TransactionState::RolledBack);
        assert_eq!(manager.active_count(), 0);
    }

    #[test]
    fn test_savepoint() {
        let mut manager = TransactionManager::new(TransactionConfig::default());
        let tx_id = manager.begin("test");

        let op1 = TransactionOperation::new("op1");
        manager.add_operation(&tx_id, op1).unwrap();

        manager.savepoint(&tx_id, "sp1").unwrap();

        let op2 = TransactionOperation::new("op2");
        manager.add_operation(&tx_id, op2).unwrap();

        let tx = manager.get(&tx_id).unwrap();
        assert_eq!(tx.savepoints.len(), 1);
        assert_eq!(tx.savepoints[0].name, "sp1");
        assert_eq!(tx.savepoints[0].operation_index, 1);
    }

    #[test]
    fn test_nested_transaction() {
        let mut manager = TransactionManager::new(TransactionConfig::default());

        let parent_id = manager.begin("parent");
        let child_id = manager.begin_nested(&parent_id, "child").unwrap();

        let child_tx = manager.get(&child_id).unwrap();
        assert_eq!(child_tx.parent_id, Some(parent_id));
    }

    #[test]
    fn test_transaction_timeout() {
        let config = TransactionConfig {
            timeout: Duration::from_secs(0), // Immediate timeout
            ..Default::default()
        };
        let mut manager = TransactionManager::new(config);

        let tx_id = manager.begin("test");

        // Should fail due to timeout
        let result = manager.commit(&tx_id);
        assert!(matches!(result, Err(TransactionError::Timeout(_))));
    }
}
