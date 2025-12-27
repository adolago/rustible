# Distributed Execution Architecture Design

**Document Version**: 1.0
**Status**: Draft
**Author**: Architecture Team
**Date**: 2025-12-26
**FEAT**: FEAT-10

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Problem Statement](#2-problem-statement)
3. [Architecture Overview](#3-architecture-overview)
4. [Multi-Controller Architecture](#4-multi-controller-architecture)
5. [Work Distribution Protocol](#5-work-distribution-protocol)
6. [State Synchronization](#6-state-synchronization)
7. [Failure Recovery](#7-failure-recovery)
8. [Load Balancing](#8-load-balancing)
9. [Architecture Decision Records](#9-architecture-decision-records)
10. [Security Considerations](#10-security-considerations)
11. [Future Considerations](#11-future-considerations)

---

## 1. Executive Summary

This document describes the distributed execution architecture for Rustible, enabling horizontal scaling of playbook execution across multiple controller nodes. The design addresses the need to manage large-scale infrastructure (10,000+ hosts) while maintaining consistency, fault tolerance, and operational simplicity.

### Key Design Goals

- **Horizontal Scalability**: Support execution across multiple controller nodes
- **Fault Tolerance**: Automatic recovery from controller and target host failures
- **Consistency**: Guaranteed state synchronization across controllers
- **Performance**: Minimize coordination overhead while maximizing throughput
- **Operational Simplicity**: Easy deployment and management

---

## 2. Problem Statement

### Current Limitations

The existing Rustible architecture operates with a single controller model:

```
                    +------------------+
                    |   Controller     |
                    |   (Single Node)  |
                    +--------+---------+
                             |
            +----------------+----------------+
            |                |                |
      +-----v----+     +-----v----+     +-----v----+
      |  Host 1  |     |  Host 2  |     |  Host N  |
      +----------+     +----------+     +----------+
```

**Challenges with Single Controller**:

1. **Scalability Ceiling**: Single controller limits concurrent host connections
2. **Single Point of Failure**: Controller failure halts all execution
3. **Resource Constraints**: Memory/CPU limits on single node
4. **Geographic Distribution**: High latency to distant hosts

### Target Scale

| Metric | Current | Target |
|--------|---------|--------|
| Concurrent Hosts | ~500 | 10,000+ |
| Controllers | 1 | 3-10 |
| Recovery Time | Manual | < 30 seconds |
| Geographic Regions | 1 | Multiple |

---

## 3. Architecture Overview

### C4 Context Diagram

```
+------------------------------------------------------------------+
|                        Rustible Distributed System                |
+------------------------------------------------------------------+
|                                                                    |
|   +------------------+    +------------------+    +------------+  |
|   |   Control Plane  |    |   Data Plane     |    | Observability|
|   |                  |    |                  |    |              |
|   | - Leader Election|    | - Task Execution |    | - Metrics   |
|   | - Work Assignment|    | - State Updates  |    | - Tracing   |
|   | - Health Checks  |    | - File Transfer  |    | - Logging   |
|   +------------------+    +------------------+    +------------+  |
|                                                                    |
+------------------------------------------------------------------+
                              |
          +-------------------+-------------------+
          |                   |                   |
    +-----v-----+       +-----v-----+       +-----v-----+
    |  Region A |       |  Region B |       |  Region C |
    |  Targets  |       |  Targets  |       |  Targets  |
    +-----------+       +-----------+       +-----------+
```

### Component Overview

```
+-------------------------------------------------------------------+
|                     Distributed Controller Cluster                 |
+-------------------------------------------------------------------+
|                                                                    |
|  +-----------+     +-----------+     +-----------+                |
|  |Controller1|     |Controller2|     |Controller3|                |
|  |  (Leader) |<--->| (Follower)|<--->| (Follower)|                |
|  +-----------+     +-----------+     +-----------+                |
|       |                 |                 |                        |
|       +--------+--------+--------+--------+                        |
|                |                 |                                 |
|         +------v------+   +------v------+                         |
|         | Consensus   |   |   Shared    |                         |
|         | Protocol    |   |   State     |                         |
|         | (Raft)      |   |   Store     |                         |
|         +-------------+   +-------------+                         |
|                                                                    |
+-------------------------------------------------------------------+
```

---

## 4. Multi-Controller Architecture

### 4.1 Controller Roles

#### Primary Controller (Leader)

Responsibilities:
- Playbook parsing and validation
- Work unit creation and assignment
- Global state coordination
- Cluster membership management
- Client request handling

#### Secondary Controllers (Followers)

Responsibilities:
- Execute assigned work units
- Maintain local state cache
- Participate in leader election
- Handle regional execution
- Provide read-only API access

### 4.2 Controller Node Structure

```
+------------------------------------------------------------------+
|                        Controller Node                            |
+------------------------------------------------------------------+
|                                                                    |
|  +------------------+  +------------------+  +-----------------+  |
|  |   API Server     |  |  Work Executor   |  | State Manager   |  |
|  |                  |  |                  |  |                 |  |
|  | - REST/gRPC API  |  | - Task Runner    |  | - Local Cache   |  |
|  | - WebSocket      |  | - Connection Pool|  | - Sync Engine   |  |
|  | - Auth/AuthZ     |  | - Parallelization|  | - Persistence   |  |
|  +------------------+  +------------------+  +-----------------+  |
|                                                                    |
|  +------------------+  +------------------+  +-----------------+  |
|  | Cluster Manager  |  |  Health Monitor  |  | Event Bus       |  |
|  |                  |  |                  |  |                 |  |
|  | - Leader Election|  | - Self Health    |  | - Pub/Sub       |  |
|  | - Membership     |  | - Peer Health    |  | - Event Queue   |  |
|  | - Partitioning   |  | - Target Health  |  | - Notifications |  |
|  +------------------+  +------------------+  +-----------------+  |
|                                                                    |
+------------------------------------------------------------------+
```

### 4.3 Leader Election Protocol

Using Raft consensus for leader election:

```
State Machine:
+------------+     Timeout      +------------+     Votes      +------------+
|  Follower  | --------------> |  Candidate | ------------> |   Leader   |
+------------+                  +------------+                +------------+
      ^                               |                            |
      |        Higher Term           |                            |
      +------------------------------+                            |
      |                                       Heartbeat Timeout   |
      +-----------------------------------------------------------+

Election Process:
1. Follower times out (150-300ms randomized)
2. Becomes candidate, increments term
3. Votes for self, requests votes from peers
4. Majority votes -> becomes leader
5. Leader sends heartbeats to maintain authority
```

**Rust Implementation Sketch**:

```rust
/// Controller role in the cluster
#[derive(Debug, Clone, PartialEq)]
pub enum ControllerRole {
    Leader,
    Follower,
    Candidate,
}

/// Raft-based cluster state
pub struct ClusterState {
    /// Current term
    current_term: u64,
    /// Controller ID that received vote in current term
    voted_for: Option<ControllerId>,
    /// Current role
    role: ControllerRole,
    /// Leader ID (if known)
    leader_id: Option<ControllerId>,
    /// Cluster members
    members: Vec<ControllerInfo>,
}

/// Controller information
pub struct ControllerInfo {
    id: ControllerId,
    address: SocketAddr,
    region: Option<String>,
    last_heartbeat: Instant,
    state: ControllerHealth,
}
```

### 4.4 Cluster Topology Modes

#### Mode 1: Active-Passive (Simple HA)

```
+------------------+         +------------------+
|   Primary        |   Sync  |   Standby        |
|   Controller     |-------->|   Controller     |
|   (Active)       |         |   (Passive)      |
+------------------+         +------------------+
        |
        v
    [All Hosts]
```

- **Use Case**: Simple high availability
- **Failover**: Automatic promotion of standby
- **State Sync**: Synchronous replication

#### Mode 2: Active-Active (Load Distribution)

```
+------------------+    +------------------+    +------------------+
|   Controller 1   |    |   Controller 2   |    |   Controller 3   |
|   (Region A)     |    |   (Region B)     |    |   (Region C)     |
+------------------+    +------------------+    +------------------+
        |                       |                       |
        v                       v                       v
   [Hosts A1-A100]         [Hosts B1-B100]         [Hosts C1-C100]
```

- **Use Case**: Geographic distribution, scale
- **Work Distribution**: Region-based affinity
- **State Sync**: Asynchronous with conflict resolution

#### Mode 3: Hierarchical (Large Scale)

```
                    +------------------+
                    |   Coordinator    |
                    |   Controller     |
                    +--------+---------+
                             |
         +-------------------+-------------------+
         |                   |                   |
  +------v-------+    +------v-------+    +------v-------+
  |  Regional    |    |  Regional    |    |  Regional    |
  |  Controller 1|    |  Controller 2|    |  Controller 3|
  +------+-------+    +------+-------+    +------+-------+
         |                   |                   |
   +-----+-----+       +-----+-----+       +-----+-----+
   |     |     |       |     |     |       |     |     |
  [H1]  [H2]  [H3]    [H4]  [H5]  [H6]    [H7]  [H8]  [H9]
```

- **Use Case**: Very large scale (10,000+ hosts)
- **Coordination**: Two-level work distribution
- **State Sync**: Hierarchical aggregation

---

## 5. Work Distribution Protocol

### 5.1 Work Unit Definition

```rust
/// A unit of work for distributed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkUnit {
    /// Unique identifier
    pub id: WorkUnitId,
    /// Parent playbook run ID
    pub run_id: RunId,
    /// Play index within playbook
    pub play_index: usize,
    /// Target hosts for this work unit
    pub hosts: Vec<HostId>,
    /// Tasks to execute
    pub tasks: Vec<TaskSpec>,
    /// Dependencies on other work units
    pub dependencies: Vec<WorkUnitId>,
    /// Priority (higher = more urgent)
    pub priority: u32,
    /// Deadline for completion
    pub deadline: Option<Instant>,
    /// Assigned controller (None = unassigned)
    pub assigned_to: Option<ControllerId>,
    /// Current state
    pub state: WorkUnitState,
    /// Retry count
    pub retries: u32,
}

/// Work unit lifecycle states
#[derive(Debug, Clone, PartialEq)]
pub enum WorkUnitState {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed { error: String },
    Cancelled,
}
```

### 5.2 Work Distribution Algorithm

```
Work Distribution Flow:

1. PARTITION: Split playbook into work units
   - By play boundaries
   - By serial batches
   - By host groups (configurable chunk size)

2. SCHEDULE: Order work units by dependencies
   - Build DAG of dependencies
   - Topological sort for execution order
   - Identify parallelizable work units

3. ASSIGN: Distribute to controllers
   - Consider controller capacity
   - Consider host affinity
   - Consider geographic proximity
   - Balance load across controllers

4. EXECUTE: Controllers process assigned work
   - Pull-based with heartbeat
   - Push results to coordinator
   - Handle failures locally when possible

5. AGGREGATE: Collect and merge results
   - Combine per-host results
   - Update global state
   - Trigger dependent work units
```

### 5.3 Assignment Strategies

#### Strategy 1: Round-Robin

```rust
pub struct RoundRobinAssigner {
    controllers: Vec<ControllerId>,
    next_index: AtomicUsize,
}

impl WorkAssigner for RoundRobinAssigner {
    fn assign(&self, work_unit: &WorkUnit) -> ControllerId {
        let idx = self.next_index.fetch_add(1, Ordering::SeqCst);
        self.controllers[idx % self.controllers.len()].clone()
    }
}
```

- **Pros**: Simple, even distribution
- **Cons**: Ignores locality, capacity

#### Strategy 2: Capacity-Aware

```rust
pub struct CapacityAwareAssigner {
    controller_loads: DashMap<ControllerId, ControllerLoad>,
}

impl WorkAssigner for CapacityAwareAssigner {
    fn assign(&self, work_unit: &WorkUnit) -> ControllerId {
        // Find controller with lowest load ratio
        self.controller_loads
            .iter()
            .min_by(|a, b| {
                let ratio_a = a.current_load as f64 / a.capacity as f64;
                let ratio_b = b.current_load as f64 / b.capacity as f64;
                ratio_a.partial_cmp(&ratio_b).unwrap()
            })
            .map(|entry| entry.key().clone())
            .expect("No controllers available")
    }
}
```

- **Pros**: Balances load, respects capacity
- **Cons**: Requires accurate load reporting

#### Strategy 3: Affinity-Based

```rust
pub struct AffinityAssigner {
    host_controller_map: DashMap<HostId, ControllerId>,
    region_controller_map: HashMap<Region, Vec<ControllerId>>,
}

impl WorkAssigner for AffinityAssigner {
    fn assign(&self, work_unit: &WorkUnit) -> ControllerId {
        // Check for existing host affinity
        if let Some(host) = work_unit.hosts.first() {
            if let Some(controller) = self.host_controller_map.get(host) {
                return controller.clone();
            }
        }

        // Fall back to region-based assignment
        if let Some(region) = work_unit.get_region() {
            if let Some(controllers) = self.region_controller_map.get(&region) {
                return self.select_by_load(controllers);
            }
        }

        // Default to least loaded controller
        self.select_least_loaded()
    }
}
```

- **Pros**: Optimizes for locality, reduces latency
- **Cons**: Can create hot spots

### 5.4 Work Queue Protocol

```
Message Types:

1. WORK_OFFER (Leader -> Follower)
   - work_unit_id
   - estimated_complexity
   - deadline
   - required_capabilities

2. WORK_ACCEPT (Follower -> Leader)
   - work_unit_id
   - estimated_completion_time

3. WORK_REJECT (Follower -> Leader)
   - work_unit_id
   - reason (capacity, capability, etc.)

4. WORK_PROGRESS (Follower -> Leader)
   - work_unit_id
   - completed_hosts
   - failed_hosts
   - current_task

5. WORK_COMPLETE (Follower -> Leader)
   - work_unit_id
   - results_summary
   - detailed_results_location

6. WORK_FAILED (Follower -> Leader)
   - work_unit_id
   - error
   - recoverable: bool
   - partial_results
```

---

## 6. State Synchronization

### 6.1 State Categories

| Category | Consistency | Sync Method | Example |
|----------|-------------|-------------|---------|
| Critical | Strong | Raft Log | Leader election, work assignment |
| Operational | Eventual | CRDT | Host facts, variable cache |
| Ephemeral | None | Local | Connection pools, metrics |

### 6.2 Distributed State Store

```
+------------------------------------------------------------------+
|                     Distributed State Store                       |
+------------------------------------------------------------------+
|                                                                    |
|  +------------------+  +------------------+  +-----------------+  |
|  |  Raft Log        |  |  Facts Store     |  | Results Store   |  |
|  |  (Consensus)     |  |  (CRDT)          |  | (Append-Only)   |  |
|  |                  |  |                  |  |                 |  |
|  | - Leader state   |  | - Host facts     |  | - Task results  |  |
|  | - Work units     |  | - Variables      |  | - Change log    |  |
|  | - Membership     |  | - Inventory      |  | - Audit trail   |  |
|  +------------------+  +------------------+  +-----------------+  |
|                                                                    |
+------------------------------------------------------------------+
```

### 6.3 CRDT-Based Facts Synchronization

Using CRDTs (Conflict-free Replicated Data Types) for eventual consistency:

```rust
/// CRDT wrapper for host facts
pub struct FactsCRDT {
    /// LWW-Map: Last-Writer-Wins Map
    facts: LWWMap<HostId, FactSet>,
    /// Vector clock for ordering
    clock: VectorClock,
}

/// Last-Writer-Wins Map implementation
pub struct LWWMap<K, V> {
    entries: HashMap<K, LWWEntry<V>>,
}

pub struct LWWEntry<V> {
    value: V,
    timestamp: HLC, // Hybrid Logical Clock
    node_id: ControllerId,
}

impl<K: Eq + Hash, V> LWWMap<K, V> {
    /// Merge two LWW-Maps
    pub fn merge(&mut self, other: &Self) {
        for (key, other_entry) in &other.entries {
            match self.entries.get(key) {
                Some(entry) if entry.timestamp >= other_entry.timestamp => {
                    // Keep existing entry
                }
                _ => {
                    // Take other entry
                    self.entries.insert(key.clone(), other_entry.clone());
                }
            }
        }
    }
}
```

### 6.4 Synchronization Protocol

```
Sync Message Flow:

Controller A                    Controller B
     |                               |
     |--- SYNC_REQUEST ------------->|
     |    (vector_clock, delta_only) |
     |                               |
     |<-- SYNC_RESPONSE -------------|
     |    (delta_facts, new_clock)   |
     |                               |
     |--- SYNC_ACK ----------------->|
     |    (merged_clock)             |
     |                               |

Anti-Entropy Protocol:
- Periodic full sync every 5 minutes
- Delta sync on every heartbeat (1 second)
- Immediate sync on critical state changes
```

### 6.5 Consistency Guarantees

```rust
/// Consistency level for operations
pub enum ConsistencyLevel {
    /// Read from any controller, may be stale
    Eventual,
    /// Read from leader or recent follower
    Session,
    /// Read from leader only
    Strong,
    /// Read from quorum of controllers
    Quorum,
}

/// Read operation with consistency level
pub async fn read_facts(
    host: &HostId,
    consistency: ConsistencyLevel,
) -> Result<FactSet, Error> {
    match consistency {
        ConsistencyLevel::Eventual => {
            // Read from local cache
            self.local_cache.get(host)
        }
        ConsistencyLevel::Session => {
            // Read from local if fresh, else from leader
            if self.local_cache.is_fresh(host, Duration::from_secs(5)) {
                self.local_cache.get(host)
            } else {
                self.fetch_from_leader(host).await
            }
        }
        ConsistencyLevel::Strong => {
            // Always read from leader
            self.fetch_from_leader(host).await
        }
        ConsistencyLevel::Quorum => {
            // Read from majority of controllers
            self.fetch_from_quorum(host).await
        }
    }
}
```

---

## 7. Failure Recovery

### 7.1 Failure Categories

```
+------------------------------------------------------------------+
|                     Failure Categories                            |
+------------------------------------------------------------------+
|                                                                    |
|  Controller Failures:                                              |
|  +----------------+  +----------------+  +----------------+       |
|  | Leader Crash   |  | Follower Crash |  | Network Split  |       |
|  | Recovery: 30s  |  | Recovery: 0s   |  | Recovery: Var  |       |
|  +----------------+  +----------------+  +----------------+       |
|                                                                    |
|  Target Host Failures:                                            |
|  +----------------+  +----------------+  +----------------+       |
|  | Connection     |  | Command        |  | Host           |       |
|  | Failure        |  | Timeout        |  | Unreachable    |       |
|  | Retry: 3x      |  | Retry: 2x      |  | Skip/Fail      |       |
|  +----------------+  +----------------+  +----------------+       |
|                                                                    |
|  Work Unit Failures:                                              |
|  +----------------+  +----------------+  +----------------+       |
|  | Partial        |  | Complete       |  | Dependency     |       |
|  | Failure        |  | Failure        |  | Failure        |       |
|  | Continue/Retry |  | Reassign       |  | Cascade/Skip   |       |
|  +----------------+  +----------------+  +----------------+       |
|                                                                    |
+------------------------------------------------------------------+
```

### 7.2 Leader Failure Recovery

```
Leader Failure Timeline:

T+0:    Leader fails
T+150ms: First follower times out (randomized 150-300ms)
T+200ms: Election begins, votes requested
T+250ms: Votes collected (majority required)
T+300ms: New leader elected
T+350ms: New leader broadcasts leadership
T+400ms: Work units reassigned from failed leader
T+500ms: Execution resumes

Total Recovery Time: ~500ms (typical)
```

**Implementation**:

```rust
pub struct LeaderRecovery {
    /// Work units that were assigned to failed leader
    orphaned_work: Vec<WorkUnit>,
    /// Last known state of orphaned work
    last_known_state: HashMap<WorkUnitId, WorkUnitState>,
}

impl LeaderRecovery {
    pub async fn recover_from_leader_failure(
        &self,
        failed_leader: ControllerId,
        cluster: &ClusterState,
    ) -> Result<(), RecoveryError> {
        // 1. Identify orphaned work units
        let orphaned = self.identify_orphaned_work(failed_leader).await?;

        // 2. Determine work unit states from replicated log
        let states = self.reconstruct_states(&orphaned).await?;

        // 3. Reassign incomplete work units
        for work_unit in orphaned {
            match states.get(&work_unit.id) {
                Some(WorkUnitState::Running) => {
                    // May need to check partial progress
                    self.reassign_with_checkpoint(work_unit).await?;
                }
                Some(WorkUnitState::Pending) => {
                    // Simply reassign
                    self.reassign(work_unit).await?;
                }
                Some(WorkUnitState::Completed) => {
                    // No action needed, results should be replicated
                }
                _ => {
                    // Retry from beginning
                    self.reset_and_reassign(work_unit).await?;
                }
            }
        }

        Ok(())
    }
}
```

### 7.3 Work Unit Checkpointing

```rust
/// Checkpoint for resumable execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkUnitCheckpoint {
    /// Work unit ID
    pub work_unit_id: WorkUnitId,
    /// Timestamp of checkpoint
    pub timestamp: Instant,
    /// Completed hosts
    pub completed_hosts: Vec<HostId>,
    /// Failed hosts (with errors)
    pub failed_hosts: HashMap<HostId, String>,
    /// Current task index per host
    pub host_task_index: HashMap<HostId, usize>,
    /// Accumulated results
    pub partial_results: HashMap<HostId, Vec<TaskResult>>,
    /// Handler notifications
    pub pending_handlers: HashSet<String>,
}

impl WorkUnitCheckpoint {
    /// Create checkpoint from current execution state
    pub fn from_execution(exec: &WorkUnitExecution) -> Self {
        Self {
            work_unit_id: exec.work_unit.id.clone(),
            timestamp: Instant::now(),
            completed_hosts: exec.completed_hosts.clone(),
            failed_hosts: exec.failed_hosts.clone(),
            host_task_index: exec.host_task_index.clone(),
            partial_results: exec.results.clone(),
            pending_handlers: exec.pending_handlers.clone(),
        }
    }

    /// Resume execution from checkpoint
    pub fn resume(&self, work_unit: WorkUnit) -> WorkUnitExecution {
        WorkUnitExecution {
            work_unit,
            completed_hosts: self.completed_hosts.clone(),
            failed_hosts: self.failed_hosts.clone(),
            host_task_index: self.host_task_index.clone(),
            results: self.partial_results.clone(),
            pending_handlers: self.pending_handlers.clone(),
            resumed_from_checkpoint: true,
        }
    }
}
```

### 7.4 Network Partition Handling

```
Partition Scenarios:

Scenario 1: Minority Partition (Safe)
+----------+    X    +----------+----------+
| C1 (old  |    X    | C2       | C3       |
| leader)  |    X    | (new     |          |
|          |    X    | leader)  |          |
+----------+    X    +----------+----------+
   Stalls           Continues with quorum

Scenario 2: Symmetric Split (Unsafe)
+----------+    X    +----------+
| C1       |    X    | C2       |
| C2       |    X    | C3       |
+----------+    X    +----------+
   Neither has majority -> all stall

Resolution Strategy:
1. Detect partition via heartbeat failures
2. Check if local partition has quorum
3. If quorum: continue operations
4. If no quorum: enter read-only mode
5. On heal: merge states, resolve conflicts
```

**Partition Detection**:

```rust
pub struct PartitionDetector {
    members: Vec<ControllerId>,
    heartbeat_failures: DashMap<ControllerId, u32>,
    partition_threshold: u32,
}

impl PartitionDetector {
    pub fn check_partition(&self) -> PartitionState {
        let reachable_count = self.members.iter()
            .filter(|id| {
                self.heartbeat_failures
                    .get(id)
                    .map(|f| *f < self.partition_threshold)
                    .unwrap_or(true)
            })
            .count();

        let total = self.members.len();
        let quorum = total / 2 + 1;

        if reachable_count >= quorum {
            PartitionState::Healthy
        } else if reachable_count > 0 {
            PartitionState::MinorityPartition
        } else {
            PartitionState::Isolated
        }
    }
}
```

### 7.5 Idempotency and Replay Safety

```rust
/// Idempotency key for operations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey {
    /// Work unit ID
    work_unit_id: WorkUnitId,
    /// Host ID
    host_id: HostId,
    /// Task index
    task_index: usize,
    /// Loop iteration (if applicable)
    loop_index: Option<usize>,
}

/// Idempotency tracker to prevent duplicate execution
pub struct IdempotencyTracker {
    /// Executed operations with their results
    executed: DashMap<IdempotencyKey, TaskResult>,
    /// TTL for entries
    ttl: Duration,
}

impl IdempotencyTracker {
    /// Check if operation was already executed
    pub fn check(&self, key: &IdempotencyKey) -> Option<TaskResult> {
        self.executed.get(key).map(|r| r.clone())
    }

    /// Record successful execution
    pub fn record(&self, key: IdempotencyKey, result: TaskResult) {
        self.executed.insert(key, result);
    }
}
```

---

## 8. Load Balancing

### 8.1 Load Metrics

```rust
/// Controller load metrics
#[derive(Debug, Clone)]
pub struct ControllerLoad {
    /// Number of active work units
    pub active_work_units: usize,
    /// Number of active host connections
    pub active_connections: usize,
    /// CPU usage percentage
    pub cpu_usage: f32,
    /// Memory usage percentage
    pub memory_usage: f32,
    /// Network bandwidth usage
    pub bandwidth_usage: f32,
    /// Average task latency
    pub avg_latency: Duration,
    /// Queue depth (pending work units)
    pub queue_depth: usize,
    /// Estimated capacity (hosts)
    pub capacity: usize,
}

impl ControllerLoad {
    /// Calculate composite load score (0.0 - 1.0)
    pub fn load_score(&self) -> f64 {
        let connection_load = self.active_connections as f64 / self.capacity as f64;
        let cpu_load = self.cpu_usage as f64 / 100.0;
        let memory_load = self.memory_usage as f64 / 100.0;
        let queue_load = (self.queue_depth as f64 / 100.0).min(1.0);

        // Weighted average
        connection_load * 0.4 + cpu_load * 0.2 + memory_load * 0.2 + queue_load * 0.2
    }
}
```

### 8.2 Load Balancing Strategies

#### Static Load Balancing

```
Host Assignment (Pre-computed):

Controller 1: Hosts 1-100    (Region A)
Controller 2: Hosts 101-200  (Region B)
Controller 3: Hosts 201-300  (Region C)

Pros: Predictable, no runtime overhead
Cons: Inflexible, can lead to imbalance
```

#### Dynamic Load Balancing

```
Real-time Rebalancing:

1. Monitor controller loads every 5 seconds
2. If imbalance > 20%:
   - Identify overloaded controller
   - Select work units to migrate
   - Assign to underloaded controllers
3. Work unit migration:
   - For pending: immediate reassignment
   - For running: wait for completion or checkpoint

Threshold-based Triggers:
- CPU > 80%: Stop accepting new work
- Memory > 85%: Trigger work migration
- Queue > 50: Request load redistribution
```

### 8.3 Load Balancer Implementation

```rust
pub struct LoadBalancer {
    controllers: Vec<ControllerId>,
    loads: DashMap<ControllerId, ControllerLoad>,
    config: LoadBalancerConfig,
}

pub struct LoadBalancerConfig {
    /// Rebalance check interval
    check_interval: Duration,
    /// Maximum load imbalance before rebalancing
    imbalance_threshold: f64,
    /// Minimum work units before considering migration
    min_migration_batch: usize,
}

impl LoadBalancer {
    /// Get optimal controller for new work unit
    pub fn select_controller(
        &self,
        work_unit: &WorkUnit,
        strategy: LoadBalanceStrategy,
    ) -> ControllerId {
        match strategy {
            LoadBalanceStrategy::LeastLoaded => {
                self.select_least_loaded()
            }
            LoadBalanceStrategy::RoundRobin => {
                self.select_round_robin()
            }
            LoadBalanceStrategy::Affinity => {
                self.select_by_affinity(work_unit)
            }
            LoadBalanceStrategy::Adaptive => {
                // Combine multiple signals
                let base = self.select_least_loaded();
                if let Some(affinity) = self.get_affinity(work_unit) {
                    // Use affinity if load is acceptable
                    if self.load_acceptable(&affinity) {
                        return affinity;
                    }
                }
                base
            }
        }
    }

    /// Check and perform rebalancing if needed
    pub async fn rebalance(&self) -> Result<RebalanceResult, Error> {
        let loads: Vec<_> = self.loads.iter()
            .map(|e| (*e.key(), e.value().load_score()))
            .collect();

        let avg_load: f64 = loads.iter().map(|(_, l)| l).sum::<f64>()
            / loads.len() as f64;

        let max_load = loads.iter().map(|(_, l)| *l).fold(0.0, f64::max);
        let min_load = loads.iter().map(|(_, l)| *l).fold(1.0, f64::min);

        let imbalance = max_load - min_load;

        if imbalance > self.config.imbalance_threshold {
            self.perform_rebalance(&loads, avg_load).await
        } else {
            Ok(RebalanceResult::NotNeeded)
        }
    }
}
```

### 8.4 Connection Pool Sharing

```rust
/// Shared connection pool across work units on same controller
pub struct SharedConnectionPool {
    /// Per-host connection pools
    pools: DashMap<HostId, RusshConnectionPool>,
    /// Maximum connections per host
    max_per_host: usize,
    /// Global maximum connections
    global_max: usize,
    /// Current total connections
    total_connections: AtomicUsize,
}

impl SharedConnectionPool {
    /// Get or create connection for host
    pub async fn get_connection(
        &self,
        host: &HostId,
    ) -> Result<PooledConnectionHandle, ConnectionError> {
        // Check if we have capacity
        if self.total_connections.load(Ordering::SeqCst) >= self.global_max {
            // Try to evict least-recently-used connection
            self.evict_lru().await?;
        }

        // Get or create pool for host
        let pool = self.pools.entry(host.clone())
            .or_insert_with(|| self.create_pool(host));

        pool.get().await
    }
}
```

---

## 9. Architecture Decision Records

### ADR-001: Raft for Consensus

**Status**: Accepted

**Context**: We need a consensus protocol for leader election and critical state replication in the distributed controller cluster.

**Decision**: Use Raft consensus protocol for:
- Leader election
- Work unit assignment log
- Cluster membership changes

**Alternatives Considered**:
- Paxos: More complex, harder to implement correctly
- ZAB (Zookeeper): Requires external dependency
- Custom leader election: Less proven, more risk

**Consequences**:
- Predictable leader election (150-300ms)
- Strong consistency for critical operations
- Requires odd number of controllers for optimal quorum
- Additional complexity vs single-controller

### ADR-002: CRDTs for Facts

**Status**: Accepted

**Context**: Host facts need to be shared across controllers but don't require strong consistency.

**Decision**: Use CRDTs (specifically LWW-Map) for facts storage:
- Eventual consistency acceptable for facts
- No coordination required for updates
- Automatic conflict resolution

**Alternatives Considered**:
- Raft replication: Overkill, adds latency
- Last-write-wins without CRDT: Possible data loss
- Operational transforms: Complex for our use case

**Consequences**:
- Facts may be temporarily inconsistent (seconds)
- No coordination overhead for fact updates
- Merge is automatic and deterministic

### ADR-003: Pull-Based Work Distribution

**Status**: Accepted

**Context**: Controllers need to receive work assignments from the leader.

**Decision**: Use pull-based work distribution:
- Controllers request work when capacity available
- Leader maintains work queue
- Heartbeat includes capacity information

**Alternatives Considered**:
- Push-based: Risk of overloading controllers
- Hybrid: Added complexity
- Dedicated work queue (Redis/RabbitMQ): External dependency

**Consequences**:
- Controllers self-regulate based on capacity
- Slightly higher latency for work assignment
- Simpler failure handling (no ack required from leader)

### ADR-004: Checkpoint-Based Recovery

**Status**: Accepted

**Context**: Work units may be interrupted by controller failures.

**Decision**: Implement checkpoint-based recovery:
- Periodic checkpoints during execution
- Checkpoints stored in distributed state
- Resume from last checkpoint on failure

**Alternatives Considered**:
- Restart from beginning: Wastes work, slow
- Transaction log replay: Complex, storage intensive
- No recovery (fail fast): Poor user experience

**Consequences**:
- Minimal work loss on failure
- Storage overhead for checkpoints
- Need to ensure checkpoint frequency balances durability vs overhead

---

## 10. Security Considerations

### 10.1 Controller Authentication

```rust
/// Controller authentication configuration
pub struct ControllerAuth {
    /// Mutual TLS for controller-to-controller
    mtls: MtlsConfig,
    /// Pre-shared key for bootstrap
    psk: Option<SecretKey>,
    /// Certificate rotation policy
    cert_rotation: CertRotationPolicy,
}

/// mTLS configuration
pub struct MtlsConfig {
    /// CA certificate for verification
    ca_cert: Certificate,
    /// Controller certificate
    controller_cert: Certificate,
    /// Controller private key
    controller_key: PrivateKey,
    /// Allowed controller identities
    allowed_identities: Vec<String>,
}
```

### 10.2 Work Unit Security

```rust
/// Security context for work unit execution
pub struct WorkUnitSecurity {
    /// Maximum privilege level allowed
    max_privilege: PrivilegeLevel,
    /// Allowed modules
    allowed_modules: HashSet<String>,
    /// Denied modules
    denied_modules: HashSet<String>,
    /// Network policies
    network_policy: NetworkPolicy,
}
```

### 10.3 State Encryption

```
State Encryption:

At Rest:
- Checkpoints encrypted with AES-256-GCM
- Facts encrypted with per-host keys
- Keys stored in controller secure storage

In Transit:
- All controller communication via mTLS
- gRPC with TLS 1.3
- Perfect forward secrecy enabled

Key Management:
- Automatic key rotation every 24 hours
- Key derivation from master secret
- Hardware security module (HSM) support optional
```

---

## 11. Future Considerations

### 11.1 Planned Enhancements

1. **Geographic Awareness**
   - Multi-region deployment
   - Latency-based routing
   - Cross-region state sync optimization

2. **Auto-Scaling**
   - Kubernetes-based controller scaling
   - Scale-to-zero for cost optimization
   - Burst capacity handling

3. **Advanced Scheduling**
   - Machine learning-based prediction
   - Priority queues with preemption
   - Deadline-aware scheduling

4. **Observability**
   - Distributed tracing (OpenTelemetry)
   - Real-time dashboards
   - Anomaly detection

### 11.2 Migration Path

```
Migration from Single Controller:

Phase 1: Add HA (Week 1-2)
- Deploy standby controller
- Enable state replication
- Test failover

Phase 2: Active-Active (Week 3-4)
- Enable work distribution
- Implement load balancing
- Monitor and tune

Phase 3: Scale Out (Week 5-6)
- Add additional controllers
- Implement regional affinity
- Full distributed operation
```

---

## Appendix A: Protocol Buffers Schema

```protobuf
syntax = "proto3";
package rustible.distributed;

// Controller registration
message ControllerInfo {
    string id = 1;
    string address = 2;
    string region = 3;
    repeated string capabilities = 4;
    int32 capacity = 5;
}

// Work unit definition
message WorkUnit {
    string id = 1;
    string run_id = 2;
    repeated string hosts = 3;
    repeated TaskSpec tasks = 4;
    repeated string dependencies = 5;
    int32 priority = 6;
    int64 deadline_ms = 7;
}

// Work assignment
message WorkAssignment {
    string work_unit_id = 1;
    string controller_id = 2;
    int64 assigned_at = 3;
}

// Heartbeat message
message Heartbeat {
    string controller_id = 1;
    int64 timestamp = 2;
    ControllerLoad load = 3;
    repeated string active_work_units = 4;
}

// Controller load metrics
message ControllerLoad {
    int32 active_work_units = 1;
    int32 active_connections = 2;
    float cpu_usage = 3;
    float memory_usage = 4;
    int32 queue_depth = 5;
}
```

---

## Appendix B: Configuration Reference

```yaml
# distributed-config.yaml
distributed:
  enabled: true

  cluster:
    # Unique cluster identifier
    cluster_id: "rustible-prod"
    # Controller endpoints
    controllers:
      - address: "controller1.example.com:9000"
        region: "us-east"
      - address: "controller2.example.com:9000"
        region: "us-west"
      - address: "controller3.example.com:9000"
        region: "eu-west"
    # Raft configuration
    raft:
      election_timeout_min_ms: 150
      election_timeout_max_ms: 300
      heartbeat_interval_ms: 50

  work_distribution:
    # Assignment strategy
    strategy: "adaptive"  # round-robin, capacity, affinity, adaptive
    # Work unit size (hosts per unit)
    work_unit_size: 50
    # Checkpoint interval
    checkpoint_interval_ms: 5000

  load_balancing:
    # Check interval
    check_interval_ms: 5000
    # Imbalance threshold (0.0 - 1.0)
    imbalance_threshold: 0.2
    # Minimum batch for migration
    min_migration_batch: 5

  state_sync:
    # Full sync interval
    full_sync_interval_ms: 300000  # 5 minutes
    # Delta sync on heartbeat
    delta_sync_enabled: true
    # Consistency level default
    default_consistency: "session"

  security:
    # mTLS for controller communication
    mtls:
      enabled: true
      ca_cert: "/etc/rustible/certs/ca.pem"
      cert: "/etc/rustible/certs/controller.pem"
      key: "/etc/rustible/certs/controller-key.pem"
    # State encryption
    encryption:
      enabled: true
      key_rotation_hours: 24
```

---

## Appendix C: Monitoring Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `rustible_controllers_active` | Gauge | Number of active controllers |
| `rustible_leader_elections_total` | Counter | Total leader elections |
| `rustible_work_units_total` | Counter | Total work units processed |
| `rustible_work_units_active` | Gauge | Currently active work units |
| `rustible_work_unit_duration_seconds` | Histogram | Work unit execution time |
| `rustible_rebalance_operations_total` | Counter | Total rebalance operations |
| `rustible_checkpoints_created_total` | Counter | Total checkpoints created |
| `rustible_checkpoint_size_bytes` | Histogram | Checkpoint size distribution |
| `rustible_state_sync_duration_seconds` | Histogram | State sync duration |
| `rustible_controller_load_score` | Gauge | Controller load score (0-1) |

---

*Document End*
