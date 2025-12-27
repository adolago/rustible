//! Task Dependency Tracking
//!
//! This module provides functionality to track and analyze dependencies between
//! tasks during playbook execution. This enables:
//!
//! - Impact analysis (what tasks are affected if one fails)
//! - Execution ordering optimization
//! - Dependency visualization
//! - Cycle detection

use std::collections::{HashMap, HashSet, VecDeque};

use petgraph::algo::{tarjan_scc, toposort};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};

use super::{StateError, StateResult};

/// A node in the dependency graph representing a task
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    /// Unique task identifier
    pub id: String,
    /// Task name
    pub name: String,
    /// Host this task runs on
    pub host: String,
    /// Module used
    pub module: String,
    /// Execution sequence number
    pub sequence: u64,
}

impl DependencyNode {
    /// Create a new dependency node
    pub fn new(id: impl Into<String>, name: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            host: host.into(),
            module: String::new(),
            sequence: 0,
        }
    }
}

/// Type of dependency between tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    /// Explicit dependency (e.g., notify handler)
    Explicit,
    /// Implicit dependency (e.g., sequential execution)
    Sequential,
    /// Resource dependency (e.g., shared file)
    Resource,
    /// Variable dependency (e.g., registered variable)
    Variable,
    /// Block dependency (e.g., rescue/always)
    Block,
}

/// A dependency between two tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDependency {
    /// Source task ID (depends on)
    pub from_id: String,
    /// Target task ID (depended by)
    pub to_id: String,
    /// Type of dependency
    pub dependency_type: DependencyType,
    /// Optional description
    pub description: Option<String>,
    /// Whether this dependency is critical (failure propagates)
    pub critical: bool,
}

impl TaskDependency {
    /// Create a new task dependency
    pub fn new(
        from_id: impl Into<String>,
        to_id: impl Into<String>,
        dep_type: DependencyType,
    ) -> Self {
        Self {
            from_id: from_id.into(),
            to_id: to_id.into(),
            dependency_type: dep_type,
            description: None,
            critical: true,
        }
    }

    /// Add a description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set whether the dependency is critical
    pub fn with_critical(mut self, critical: bool) -> Self {
        self.critical = critical;
        self
    }
}

/// The dependency graph for task relationships
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// The underlying graph
    graph: DiGraph<DependencyNode, TaskDependency>,
    /// Map from task ID to node index
    node_indices: HashMap<String, NodeIndex>,
    /// Track task sequence for implicit dependencies
    sequence_counter: u64,
    /// Last task per host for sequential dependencies
    last_task_per_host: HashMap<String, String>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    /// Create a new empty dependency graph
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
            sequence_counter: 0,
            last_task_per_host: HashMap::new(),
        }
    }

    /// Add a node to the graph
    pub fn add_node(&mut self, mut node: DependencyNode) -> NodeIndex {
        // Assign sequence number
        node.sequence = self.sequence_counter;
        self.sequence_counter += 1;

        // Check if node already exists
        if let Some(&idx) = self.node_indices.get(&node.id) {
            // Update existing node
            if let Some(existing) = self.graph.node_weight_mut(idx) {
                *existing = node;
            }
            return idx;
        }

        // Add implicit sequential dependency to last task on same host
        let host = node.host.clone();
        let node_id = node.id.clone();

        let idx = self.graph.add_node(node);
        self.node_indices.insert(node_id.clone(), idx);

        // Add sequential dependency
        if let Some(last_task_id) = self.last_task_per_host.get(&host) {
            if let Some(&from_idx) = self.node_indices.get(last_task_id) {
                let dep = TaskDependency::new(last_task_id, &node_id, DependencyType::Sequential)
                    .with_critical(false);
                self.graph.add_edge(from_idx, idx, dep);
            }
        }

        self.last_task_per_host.insert(host, node_id);
        idx
    }

    /// Add an explicit dependency between tasks
    pub fn add_dependency(&mut self, dependency: TaskDependency) -> StateResult<()> {
        let from_idx = self
            .node_indices
            .get(&dependency.from_id)
            .ok_or_else(|| StateError::StateNotFound(dependency.from_id.clone()))?;
        let to_idx = self
            .node_indices
            .get(&dependency.to_id)
            .ok_or_else(|| StateError::StateNotFound(dependency.to_id.clone()))?;

        self.graph.add_edge(*from_idx, *to_idx, dependency);
        Ok(())
    }

    /// Add a variable dependency (task registers a variable that another uses)
    pub fn add_variable_dependency(
        &mut self,
        producer_id: &str,
        consumer_id: &str,
        variable: &str,
    ) -> StateResult<()> {
        let dep = TaskDependency::new(producer_id, consumer_id, DependencyType::Variable)
            .with_description(format!("Variable: {}", variable));
        self.add_dependency(dep)
    }

    /// Add a handler dependency (task notifies a handler)
    pub fn add_handler_dependency(&mut self, task_id: &str, handler_id: &str) -> StateResult<()> {
        let dep = TaskDependency::new(task_id, handler_id, DependencyType::Explicit)
            .with_description("Handler notification".to_string());
        self.add_dependency(dep)
    }

    /// Add a block dependency (rescue/always blocks)
    pub fn add_block_dependency(
        &mut self,
        block_task_id: &str,
        related_task_id: &str,
    ) -> StateResult<()> {
        let dep = TaskDependency::new(block_task_id, related_task_id, DependencyType::Block);
        self.add_dependency(dep)
    }

    /// Check for dependency cycles
    pub fn has_cycles(&self) -> bool {
        let sccs = tarjan_scc(&self.graph);
        // A cycle exists if any SCC has more than one node
        sccs.iter().any(|scc| scc.len() > 1)
    }

    /// Get all cycles in the graph
    pub fn get_cycles(&self) -> Vec<Vec<String>> {
        let sccs = tarjan_scc(&self.graph);
        sccs.into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                scc.into_iter()
                    .filter_map(|idx| self.graph.node_weight(idx).map(|n| n.id.clone()))
                    .collect()
            })
            .collect()
    }

    /// Get topological order of tasks (execution order respecting dependencies)
    pub fn get_execution_order(&self) -> StateResult<Vec<String>> {
        match toposort(&self.graph, None) {
            Ok(order) => Ok(order
                .into_iter()
                .filter_map(|idx| self.graph.node_weight(idx).map(|n| n.id.clone()))
                .collect()),
            Err(_) => Err(StateError::DependencyCycle(
                "Cannot determine execution order: dependency cycle exists".to_string(),
            )),
        }
    }

    /// Get all tasks that depend on a given task (direct and transitive)
    pub fn get_dependents(&self, task_id: &str) -> Vec<String> {
        let mut dependents = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(&start_idx) = self.node_indices.get(task_id) {
            queue.push_back(start_idx);

            while let Some(current) = queue.pop_front() {
                for neighbor in self.graph.neighbors_directed(current, Direction::Outgoing) {
                    if let Some(node) = self.graph.node_weight(neighbor) {
                        if dependents.insert(node.id.clone()) {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        dependents.into_iter().collect()
    }

    /// Get all tasks that a given task depends on (direct and transitive)
    pub fn get_dependencies(&self, task_id: &str) -> Vec<String> {
        let mut dependencies = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(&start_idx) = self.node_indices.get(task_id) {
            queue.push_back(start_idx);

            while let Some(current) = queue.pop_front() {
                for neighbor in self.graph.neighbors_directed(current, Direction::Incoming) {
                    if let Some(node) = self.graph.node_weight(neighbor) {
                        if dependencies.insert(node.id.clone()) {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        dependencies.into_iter().collect()
    }

    /// Get direct dependencies of a task
    pub fn get_direct_dependencies(&self, task_id: &str) -> Vec<TaskDependency> {
        let mut deps = Vec::new();

        if let Some(&idx) = self.node_indices.get(task_id) {
            for edge in self.graph.edges_directed(idx, Direction::Incoming) {
                deps.push(edge.weight().clone());
            }
        }

        deps
    }

    /// Get direct dependents of a task
    pub fn get_direct_dependents(&self, task_id: &str) -> Vec<TaskDependency> {
        let mut deps = Vec::new();

        if let Some(&idx) = self.node_indices.get(task_id) {
            for edge in self.graph.edges_directed(idx, Direction::Outgoing) {
                deps.push(edge.weight().clone());
            }
        }

        deps
    }

    /// Calculate impact analysis: what tasks would be affected if a task fails
    pub fn impact_analysis(&self, task_id: &str, critical_only: bool) -> ImpactAnalysis {
        let mut affected_tasks = HashSet::new();
        let mut queue = VecDeque::new();

        if let Some(&start_idx) = self.node_indices.get(task_id) {
            queue.push_back(start_idx);

            while let Some(current) = queue.pop_front() {
                for edge in self.graph.edges_directed(current, Direction::Outgoing) {
                    let dep = edge.weight();

                    // Skip non-critical dependencies if requested
                    if critical_only && !dep.critical {
                        continue;
                    }

                    let target = edge.target();
                    if let Some(node) = self.graph.node_weight(target) {
                        if affected_tasks.insert(node.id.clone()) {
                            queue.push_back(target);
                        }
                    }
                }
            }
        }

        let task_node = self
            .node_indices
            .get(task_id)
            .and_then(|idx| self.graph.node_weight(*idx).cloned());

        ImpactAnalysis {
            task_id: task_id.to_string(),
            task_name: task_node.map(|n| n.name).unwrap_or_default(),
            affected_task_ids: affected_tasks.into_iter().collect(),
            critical_path_length: self.get_critical_path_length(task_id),
        }
    }

    /// Get the length of the critical path from a task to its final dependent
    fn get_critical_path_length(&self, task_id: &str) -> usize {
        let mut max_depth = 0;

        if let Some(&start_idx) = self.node_indices.get(task_id) {
            let mut queue = VecDeque::new();
            queue.push_back((start_idx, 0));
            let mut visited = HashSet::new();

            while let Some((current, depth)) = queue.pop_front() {
                max_depth = max_depth.max(depth);

                for neighbor in self.graph.neighbors_directed(current, Direction::Outgoing) {
                    if visited.insert(neighbor) {
                        queue.push_back((neighbor, depth + 1));
                    }
                }
            }
        }

        max_depth
    }

    /// Get a node by ID
    pub fn get_node(&self, task_id: &str) -> Option<&DependencyNode> {
        self.node_indices
            .get(task_id)
            .and_then(|idx| self.graph.node_weight(*idx))
    }

    /// Get the number of nodes
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Get the number of edges
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Generate a DOT format representation for visualization
    pub fn to_dot(&self) -> String {
        let mut output = String::new();
        output.push_str("digraph dependencies {\n");
        output.push_str("  rankdir=LR;\n");
        output.push_str("  node [shape=box];\n\n");

        // Add nodes
        for idx in self.graph.node_indices() {
            if let Some(node) = self.graph.node_weight(idx) {
                let label = format!("{}\\n{}", node.name, node.host);
                output.push_str(&format!("  \"{}\" [label=\"{}\"];\n", node.id, label));
            }
        }

        output.push('\n');

        // Add edges
        for edge in self.graph.edge_references() {
            let source = self
                .graph
                .node_weight(edge.source())
                .map(|n| n.id.as_str())
                .unwrap_or("?");
            let target = self
                .graph
                .node_weight(edge.target())
                .map(|n| n.id.as_str())
                .unwrap_or("?");
            let dep = edge.weight();

            let style = match dep.dependency_type {
                DependencyType::Explicit => "solid",
                DependencyType::Sequential => "dotted",
                DependencyType::Resource => "dashed",
                DependencyType::Variable => "bold",
                DependencyType::Block => "double",
            };

            let color = if dep.critical { "red" } else { "gray" };

            output.push_str(&format!(
                "  \"{}\" -> \"{}\" [style={}, color={}];\n",
                source, target, style, color
            ));
        }

        output.push_str("}\n");
        output
    }

    /// Get tasks grouped by host
    pub fn tasks_by_host(&self) -> HashMap<String, Vec<String>> {
        let mut by_host: HashMap<String, Vec<String>> = HashMap::new();

        for idx in self.graph.node_indices() {
            if let Some(node) = self.graph.node_weight(idx) {
                by_host
                    .entry(node.host.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }

        by_host
    }
}

/// Result of an impact analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysis {
    /// Task ID that was analyzed
    pub task_id: String,
    /// Task name
    pub task_name: String,
    /// List of task IDs that would be affected by failure
    pub affected_task_ids: Vec<String>,
    /// Length of the critical path
    pub critical_path_length: usize,
}

impl ImpactAnalysis {
    /// Get the number of affected tasks
    pub fn affected_count(&self) -> usize {
        self.affected_task_ids.len()
    }

    /// Check if this task has high impact
    pub fn is_high_impact(&self, threshold: usize) -> bool {
        self.affected_task_ids.len() >= threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_creation() {
        let graph = DependencyGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_nodes() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Install nginx", "host1");
        let node2 = DependencyNode::new("task2", "Start nginx", "host1");

        graph.add_node(node1);
        graph.add_node(node2);

        assert_eq!(graph.node_count(), 2);
        // Should have one sequential edge from task1 -> task2
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_add_explicit_dependency() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("handler1", "Handler 1", "host1");

        graph.add_node(node1);
        graph.add_node(node2);

        graph.add_handler_dependency("task1", "handler1").unwrap();

        // 1 sequential + 1 explicit
        assert!(graph.edge_count() >= 2);
    }

    #[test]
    fn test_get_dependents() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host1");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        let dependents = graph.get_dependents("task1");
        assert!(dependents.contains(&"task2".to_string()));
        assert!(dependents.contains(&"task3".to_string()));
    }

    #[test]
    fn test_get_dependencies() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host1");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        let dependencies = graph.get_dependencies("task3");
        assert!(dependencies.contains(&"task1".to_string()));
        assert!(dependencies.contains(&"task2".to_string()));
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = DependencyGraph::new();

        // Create three tasks on different hosts to avoid auto sequential deps
        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host2");
        let node3 = DependencyNode::new("task3", "Task 3", "host3");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        // Create a cycle: task1 -> task2 -> task3 -> task1
        graph
            .add_dependency(TaskDependency::new(
                "task1",
                "task2",
                DependencyType::Explicit,
            ))
            .unwrap();
        graph
            .add_dependency(TaskDependency::new(
                "task2",
                "task3",
                DependencyType::Explicit,
            ))
            .unwrap();
        graph
            .add_dependency(TaskDependency::new(
                "task3",
                "task1",
                DependencyType::Explicit,
            ))
            .unwrap();

        assert!(graph.has_cycles());

        let cycles = graph.get_cycles();
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_no_cycles() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host1");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        // Linear: task1 -> task2 -> task3
        assert!(!graph.has_cycles());
    }

    #[test]
    fn test_execution_order() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host1");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        let order = graph.get_execution_order().unwrap();
        assert_eq!(order.len(), 3);
        // task1 should come before task2, task2 before task3
        let pos1 = order.iter().position(|t| t == "task1").unwrap();
        let pos2 = order.iter().position(|t| t == "task2").unwrap();
        let pos3 = order.iter().position(|t| t == "task3").unwrap();
        assert!(pos1 < pos2);
        assert!(pos2 < pos3);
    }

    #[test]
    fn test_impact_analysis() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host1");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        let impact = graph.impact_analysis("task1", false);
        assert_eq!(impact.affected_count(), 2);
    }

    #[test]
    fn test_to_dot() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Install nginx", "host1");
        let node2 = DependencyNode::new("task2", "Start nginx", "host1");

        graph.add_node(node1);
        graph.add_node(node2);

        let dot = graph.to_dot();
        assert!(dot.contains("digraph"));
        assert!(dot.contains("task1"));
        assert!(dot.contains("task2"));
    }

    #[test]
    fn test_tasks_by_host() {
        let mut graph = DependencyGraph::new();

        let node1 = DependencyNode::new("task1", "Task 1", "host1");
        let node2 = DependencyNode::new("task2", "Task 2", "host1");
        let node3 = DependencyNode::new("task3", "Task 3", "host2");

        graph.add_node(node1);
        graph.add_node(node2);
        graph.add_node(node3);

        let by_host = graph.tasks_by_host();
        assert_eq!(by_host.get("host1").unwrap().len(), 2);
        assert_eq!(by_host.get("host2").unwrap().len(), 1);
    }
}
