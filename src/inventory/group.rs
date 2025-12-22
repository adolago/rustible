//! Group definition for Rustible inventory system.
//!
//! This module provides the `Group` structure representing a logical grouping
//! of hosts with shared variables and parent-child relationships.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A group of hosts in the inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Group name
    pub name: String,

    /// Host names belonging to this group
    #[serde(default)]
    pub hosts: HashSet<String>,

    /// Child group names
    #[serde(default)]
    pub children: HashSet<String>,

    /// Parent group names (computed from children relationships)
    #[serde(skip)]
    pub parents: HashSet<String>,

    /// Group-specific variables
    #[serde(default)]
    pub vars: IndexMap<String, serde_yaml::Value>,

    /// Priority for variable precedence (higher = more priority)
    #[serde(default)]
    pub priority: i32,
}

impl Group {
    /// Create a new group with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hosts: HashSet::new(),
            children: HashSet::new(),
            parents: HashSet::new(),
            vars: IndexMap::new(),
            priority: 0,
        }
    }

    /// Create the special "all" group
    pub fn all() -> Self {
        Self::new("all")
    }

    /// Create the special "ungrouped" group
    pub fn ungrouped() -> Self {
        Self::new("ungrouped")
    }

    /// Add a host to this group
    pub fn add_host(&mut self, host: impl Into<String>) {
        self.hosts.insert(host.into());
    }

    /// Remove a host from this group
    pub fn remove_host(&mut self, host: &str) -> bool {
        self.hosts.remove(host)
    }

    /// Check if a host belongs to this group
    pub fn has_host(&self, host: &str) -> bool {
        self.hosts.contains(host)
    }

    /// Add a child group
    pub fn add_child(&mut self, child: impl Into<String>) {
        self.children.insert(child.into());
    }

    /// Remove a child group
    pub fn remove_child(&mut self, child: &str) -> bool {
        self.children.remove(child)
    }

    /// Check if a group is a child of this group
    pub fn has_child(&self, child: &str) -> bool {
        self.children.contains(child)
    }

    /// Add a parent group (internal use for reverse lookups)
    pub fn add_parent(&mut self, parent: impl Into<String>) {
        self.parents.insert(parent.into());
    }

    /// Remove a parent group
    pub fn remove_parent(&mut self, parent: &str) -> bool {
        self.parents.remove(parent)
    }

    /// Check if a group is a parent of this group
    pub fn has_parent(&self, parent: &str) -> bool {
        self.parents.contains(parent)
    }

    /// Set a variable on this group
    pub fn set_var(&mut self, key: impl Into<String>, value: serde_yaml::Value) {
        self.vars.insert(key.into(), value);
    }

    /// Get a variable from this group
    pub fn get_var(&self, key: &str) -> Option<&serde_yaml::Value> {
        self.vars.get(key)
    }

    /// Check if group has a specific variable
    pub fn has_var(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Get all host names (direct members only, not from child groups)
    pub fn direct_hosts(&self) -> impl Iterator<Item = &String> {
        self.hosts.iter()
    }

    /// Get all child group names
    pub fn child_groups(&self) -> impl Iterator<Item = &String> {
        self.children.iter()
    }

    /// Get all parent group names
    pub fn parent_groups(&self) -> impl Iterator<Item = &String> {
        self.parents.iter()
    }

    /// Check if this group is empty (no hosts and no children)
    pub fn is_empty(&self) -> bool {
        self.hosts.is_empty() && self.children.is_empty()
    }

    /// Get the number of direct host members
    pub fn host_count(&self) -> usize {
        self.hosts.len()
    }

    /// Get the number of child groups
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// Merge variables from another group (other takes precedence)
    pub fn merge_vars(&mut self, other: &IndexMap<String, serde_yaml::Value>) {
        for (key, value) in other {
            self.vars.insert(key.clone(), value.clone());
        }
    }

    /// Get depth in the group hierarchy (0 for "all", increases for each parent)
    /// This is used for variable precedence calculations
    pub fn depth(&self) -> usize {
        self.parents.len()
    }
}

impl PartialEq for Group {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Group {}

impl std::hash::Hash for Group {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl std::fmt::Display for Group {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({} hosts", self.name, self.hosts.len())?;
        if !self.children.is_empty() {
            write!(f, ", {} children", self.children.len())?;
        }
        write!(f, ")")
    }
}

/// Builder for creating groups with a fluent API
#[derive(Debug, Default)]
pub struct GroupBuilder {
    name: String,
    hosts: HashSet<String>,
    children: HashSet<String>,
    vars: IndexMap<String, serde_yaml::Value>,
    priority: i32,
}

impl GroupBuilder {
    /// Create a new group builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Add a host to the group
    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.hosts.insert(host.into());
        self
    }

    /// Add multiple hosts to the group
    pub fn hosts<I, S>(mut self, hosts: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for host in hosts {
            self.hosts.insert(host.into());
        }
        self
    }

    /// Add a child group
    pub fn child(mut self, child: impl Into<String>) -> Self {
        self.children.insert(child.into());
        self
    }

    /// Add multiple child groups
    pub fn children<I, S>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for child in children {
            self.children.insert(child.into());
        }
        self
    }

    /// Add a variable
    pub fn var(mut self, key: impl Into<String>, value: serde_yaml::Value) -> Self {
        self.vars.insert(key.into(), value);
        self
    }

    /// Set the priority
    pub fn priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Build the group
    pub fn build(self) -> Group {
        Group {
            name: self.name,
            hosts: self.hosts,
            children: self.children,
            parents: HashSet::new(),
            vars: self.vars,
            priority: self.priority,
        }
    }
}

/// Represents the group hierarchy for variable inheritance
#[derive(Debug, Clone)]
pub struct GroupHierarchy {
    /// Ordered list of groups from most specific to least specific (child to parent)
    pub groups: Vec<String>,
}

impl GroupHierarchy {
    /// Create a new empty hierarchy
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    /// Add a group to the hierarchy
    pub fn push(&mut self, group: impl Into<String>) {
        self.groups.push(group.into());
    }

    /// Get groups in order from least specific to most specific (parent to child)
    /// This is the order for variable application (later overrides earlier)
    pub fn parent_to_child(&self) -> impl Iterator<Item = &String> {
        self.groups.iter().rev()
    }

    /// Get groups in order from most specific to least specific (child to parent)
    pub fn child_to_parent(&self) -> impl Iterator<Item = &String> {
        self.groups.iter()
    }
}

impl Default for GroupHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_new() {
        let group = Group::new("webservers");
        assert_eq!(group.name, "webservers");
        assert!(group.hosts.is_empty());
        assert!(group.children.is_empty());
        assert!(group.vars.is_empty());
    }

    #[test]
    fn test_group_hosts() {
        let mut group = Group::new("webservers");
        group.add_host("web1");
        group.add_host("web2");
        assert!(group.has_host("web1"));
        assert!(group.has_host("web2"));
        assert!(!group.has_host("db1"));
        assert_eq!(group.host_count(), 2);

        group.remove_host("web1");
        assert!(!group.has_host("web1"));
        assert_eq!(group.host_count(), 1);
    }

    #[test]
    fn test_group_children() {
        let mut group = Group::new("production");
        group.add_child("webservers");
        group.add_child("databases");
        assert!(group.has_child("webservers"));
        assert!(group.has_child("databases"));
        assert_eq!(group.child_count(), 2);
    }

    #[test]
    fn test_group_vars() {
        let mut group = Group::new("webservers");
        group.set_var("http_port", serde_yaml::Value::Number(80.into()));
        assert!(group.has_var("http_port"));
        assert_eq!(
            group.get_var("http_port"),
            Some(&serde_yaml::Value::Number(80.into()))
        );
    }

    #[test]
    fn test_group_builder() {
        let group = GroupBuilder::new("webservers")
            .hosts(["web1", "web2"])
            .child("nginx")
            .var("http_port", serde_yaml::Value::Number(80.into()))
            .priority(10)
            .build();

        assert_eq!(group.name, "webservers");
        assert!(group.has_host("web1"));
        assert!(group.has_host("web2"));
        assert!(group.has_child("nginx"));
        assert!(group.has_var("http_port"));
        assert_eq!(group.priority, 10);
    }

    #[test]
    fn test_group_hierarchy() {
        let mut hierarchy = GroupHierarchy::new();
        hierarchy.push("all");
        hierarchy.push("production");
        hierarchy.push("webservers");

        let parent_to_child: Vec<_> = hierarchy.parent_to_child().collect();
        assert_eq!(parent_to_child, vec!["webservers", "production", "all"]);

        let child_to_parent: Vec<_> = hierarchy.child_to_parent().collect();
        assert_eq!(child_to_parent, vec!["all", "production", "webservers"]);
    }
}
