//! Variable inspection for debugging.
//!
//! This module provides the ability to inspect variables at any point
//! during playbook execution, with support for watches and deep inspection.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// Scope for variable inspection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableScope {
    /// Global variables (extra vars, etc.)
    Global,
    /// Play-level variables
    Play,
    /// Task-level variables
    Task,
    /// Host-specific variables
    Host,
    /// Group variables
    Group,
    /// Facts gathered from hosts
    Facts,
    /// Registered variables from previous tasks
    Registered,
    /// Loop variables (item, ansible_loop, etc.)
    Loop,
    /// Role variables
    Role,
    /// Block variables
    Block,
    /// Any scope (for searches)
    Any,
}

impl VariableScope {
    /// Get a display name for the scope
    pub fn display_name(&self) -> &'static str {
        match self {
            VariableScope::Global => "global",
            VariableScope::Play => "play",
            VariableScope::Task => "task",
            VariableScope::Host => "host",
            VariableScope::Group => "group",
            VariableScope::Facts => "facts",
            VariableScope::Registered => "registered",
            VariableScope::Loop => "loop",
            VariableScope::Role => "role",
            VariableScope::Block => "block",
            VariableScope::Any => "any",
        }
    }
}

/// Source of a variable value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSource {
    /// Scope where the variable was found
    pub scope: VariableScope,
    /// File where the variable was defined (if known)
    pub file: Option<String>,
    /// Line number in the file (if known)
    pub line: Option<usize>,
    /// Task or play name where defined
    pub context: Option<String>,
}

impl VariableSource {
    /// Create a new variable source
    pub fn new(scope: VariableScope) -> Self {
        Self {
            scope,
            file: None,
            line: None,
            context: None,
        }
    }

    /// Set the file
    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the line number
    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the context
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }
}

/// Result of inspecting a variable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    /// Name of the variable
    pub name: String,
    /// Current value
    pub value: JsonValue,
    /// Type of the value
    pub value_type: String,
    /// Source of the variable
    pub source: Option<VariableSource>,
    /// Whether the variable was found
    pub found: bool,
    /// Pretty-printed representation
    pub pretty: String,
    /// Size information (for collections)
    pub size: Option<usize>,
    /// Child keys (for objects)
    pub children: Option<Vec<String>>,
}

impl InspectionResult {
    /// Create a result for a found variable
    pub fn found(name: impl Into<String>, value: JsonValue) -> Self {
        let name = name.into();
        let value_type = Self::type_name(&value);
        let size = Self::value_size(&value);
        let children = Self::value_children(&value);
        let pretty = Self::pretty_print(&value);

        Self {
            name,
            value,
            value_type,
            source: None,
            found: true,
            pretty,
            size,
            children,
        }
    }

    /// Create a result for a not found variable
    pub fn not_found(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: JsonValue::Null,
            value_type: "undefined".to_string(),
            source: None,
            found: false,
            pretty: "<undefined>".to_string(),
            size: None,
            children: None,
        }
    }

    /// Set the source
    pub fn with_source(mut self, source: VariableSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Get the type name for a JSON value
    fn type_name(value: &JsonValue) -> String {
        match value {
            JsonValue::Null => "null".to_string(),
            JsonValue::Bool(_) => "bool".to_string(),
            JsonValue::Number(n) => {
                if n.is_i64() {
                    "int".to_string()
                } else if n.is_f64() {
                    "float".to_string()
                } else {
                    "number".to_string()
                }
            }
            JsonValue::String(_) => "string".to_string(),
            JsonValue::Array(_) => "list".to_string(),
            JsonValue::Object(_) => "dict".to_string(),
        }
    }

    /// Get the size of a value (for collections)
    fn value_size(value: &JsonValue) -> Option<usize> {
        match value {
            JsonValue::Array(arr) => Some(arr.len()),
            JsonValue::Object(obj) => Some(obj.len()),
            JsonValue::String(s) => Some(s.len()),
            _ => None,
        }
    }

    /// Get child keys for objects
    fn value_children(value: &JsonValue) -> Option<Vec<String>> {
        match value {
            JsonValue::Object(obj) => Some(obj.keys().cloned().collect()),
            _ => None,
        }
    }

    /// Pretty print a value
    fn pretty_print(value: &JsonValue) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{:?}", value))
    }
}

/// A variable watch configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableWatch {
    /// Variable name (can include dot notation for nested access)
    pub name: String,
    /// Scope to watch (None means any scope)
    pub scope: Option<VariableScope>,
    /// Whether to track changes
    pub track_changes: bool,
    /// Previous value (for change tracking)
    pub previous_value: Option<JsonValue>,
    /// Number of times the value has changed
    pub change_count: usize,
    /// Whether this is a conditional watch
    pub condition: Option<WatchCondition>,
}

impl VariableWatch {
    /// Create a new variable watch
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            scope: None,
            track_changes: true,
            previous_value: None,
            change_count: 0,
            condition: None,
        }
    }

    /// Set the scope
    pub fn with_scope(mut self, scope: VariableScope) -> Self {
        self.scope = Some(scope);
        self
    }

    /// Set change tracking
    pub fn with_change_tracking(mut self, enabled: bool) -> Self {
        self.track_changes = enabled;
        self
    }

    /// Set a watch condition
    pub fn with_condition(mut self, condition: WatchCondition) -> Self {
        self.condition = Some(condition);
        self
    }

    /// Update the watch with a new value
    pub fn update(&mut self, value: &JsonValue) -> bool {
        let changed = match &self.previous_value {
            Some(prev) => prev != value,
            None => true,
        };

        if changed {
            self.previous_value = Some(value.clone());
            self.change_count += 1;
        }

        changed
    }

    /// Check if the condition is met
    pub fn condition_met(&self, value: &JsonValue) -> bool {
        match &self.condition {
            None => true,
            Some(cond) => cond.evaluate(value),
        }
    }
}

/// Condition for a variable watch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchCondition {
    /// Value equals a specific value
    Equals(JsonValue),
    /// Value does not equal a specific value
    NotEquals(JsonValue),
    /// Value is greater than (for numbers)
    GreaterThan(f64),
    /// Value is less than (for numbers)
    LessThan(f64),
    /// Value contains a substring (for strings)
    Contains(String),
    /// Value matches a regex pattern (for strings)
    Matches(String),
    /// Value is null
    IsNull,
    /// Value is not null
    IsNotNull,
    /// Value is truthy
    IsTruthy,
    /// Value changed from previous
    Changed,
}

impl WatchCondition {
    /// Evaluate the condition against a value
    pub fn evaluate(&self, value: &JsonValue) -> bool {
        match self {
            WatchCondition::Equals(expected) => value == expected,
            WatchCondition::NotEquals(expected) => value != expected,
            WatchCondition::GreaterThan(threshold) => {
                value.as_f64().map(|v| v > *threshold).unwrap_or(false)
            }
            WatchCondition::LessThan(threshold) => {
                value.as_f64().map(|v| v < *threshold).unwrap_or(false)
            }
            WatchCondition::Contains(substr) => {
                value.as_str().map(|s| s.contains(substr)).unwrap_or(false)
            }
            WatchCondition::Matches(pattern) => {
                if let (Some(s), Ok(re)) = (value.as_str(), regex::Regex::new(pattern)) {
                    re.is_match(s)
                } else {
                    false
                }
            }
            WatchCondition::IsNull => value.is_null(),
            WatchCondition::IsNotNull => !value.is_null(),
            WatchCondition::IsTruthy => match value {
                JsonValue::Null => false,
                JsonValue::Bool(b) => *b,
                JsonValue::Number(n) => n.as_f64().map(|v| v != 0.0).unwrap_or(false),
                JsonValue::String(s) => !s.is_empty(),
                JsonValue::Array(a) => !a.is_empty(),
                JsonValue::Object(o) => !o.is_empty(),
            },
            WatchCondition::Changed => true, // Handled separately by watch update logic
        }
    }
}

/// Variable inspector for examining execution state
#[derive(Debug)]
pub struct VariableInspector {
    /// Active watches
    watches: HashMap<String, VariableWatch>,
    /// History of inspections
    history: Vec<InspectionResult>,
    /// Maximum history entries
    max_history: usize,
}

impl VariableInspector {
    /// Create a new variable inspector
    pub fn new() -> Self {
        Self {
            watches: HashMap::new(),
            history: Vec::new(),
            max_history: 1000,
        }
    }

    /// Set maximum history size
    pub fn with_max_history(mut self, max: usize) -> Self {
        self.max_history = max;
        self
    }

    /// Add a variable watch
    pub fn add_watch(&mut self, name: impl Into<String>, scope: Option<VariableScope>) {
        let name = name.into();
        let mut watch = VariableWatch::new(name.clone());
        if let Some(s) = scope {
            watch = watch.with_scope(s);
        }
        self.watches.insert(name, watch);
    }

    /// Remove a variable watch
    pub fn remove_watch(&mut self, name: &str) {
        self.watches.remove(name);
    }

    /// Clear all watches
    pub fn clear_watches(&mut self) {
        self.watches.clear();
    }

    /// Get all watches
    pub fn get_watches(&self) -> Vec<&VariableWatch> {
        self.watches.values().collect()
    }

    /// Inspect a specific variable
    pub fn inspect(
        &self,
        name: &str,
        vars: &HashMap<String, JsonValue>,
    ) -> Option<InspectionResult> {
        // Handle dot notation for nested access
        let parts: Vec<&str> = name.split('.').collect();
        let root_name = parts[0];

        if let Some(value) = vars.get(root_name) {
            // Navigate to nested value if needed
            let mut current = value.clone();
            for part in parts.iter().skip(1) {
                current = match current {
                    JsonValue::Object(ref obj) => obj.get(*part)?.clone(),
                    JsonValue::Array(ref arr) => {
                        let idx: usize = part.parse().ok()?;
                        arr.get(idx)?.clone()
                    }
                    _ => return None,
                };
            }
            Some(InspectionResult::found(name, current))
        } else {
            Some(InspectionResult::not_found(name))
        }
    }

    /// Inspect all watched variables
    pub fn inspect_watched(&self, vars: &HashMap<String, JsonValue>) -> Vec<InspectionResult> {
        self.watches
            .keys()
            .filter_map(|name| self.inspect(name, vars))
            .collect()
    }

    /// Inspect all variables
    pub fn inspect_all(&self, vars: &HashMap<String, JsonValue>) -> Vec<InspectionResult> {
        vars.iter()
            .map(|(name, value)| InspectionResult::found(name, value.clone()))
            .collect()
    }

    /// Update watches with current values and return changed ones
    pub fn update_watches(&mut self, vars: &HashMap<String, JsonValue>) -> Vec<&VariableWatch> {
        // First pass: collect names and results to avoid borrow conflicts
        let updates: Vec<(String, Option<JsonValue>)> = self
            .watches
            .keys()
            .map(|name| {
                let result = Self::inspect_vars(name, vars);
                (
                    name.clone(),
                    if result.found {
                        Some(result.value)
                    } else {
                        None
                    },
                )
            })
            .collect();

        // Second pass: apply updates
        for (name, value) in updates {
            if let (Some(watch), Some(val)) = (self.watches.get_mut(&name), value) {
                watch.update(&val);
            }
        }

        // Return watches that changed
        self.watches
            .values()
            .filter(|w| w.change_count > 0)
            .collect()
    }

    // Static helper to inspect variables without borrowing self
    fn inspect_vars(name: &str, vars: &HashMap<String, JsonValue>) -> InspectionResult {
        let parts: Vec<&str> = name.split('.').collect();
        let result = Self::lookup_nested(vars, &parts);
        if let Some(value) = result {
            InspectionResult::found(name, value)
        } else {
            InspectionResult::not_found(name)
        }
    }

    // Static nested lookup helper
    fn lookup_nested(vars: &HashMap<String, JsonValue>, parts: &[&str]) -> Option<JsonValue> {
        if parts.is_empty() {
            return None;
        }
        let first = parts[0];
        let value = vars.get(first)?;
        if parts.len() == 1 {
            return Some(value.clone());
        }
        // Navigate nested structure
        let mut current = value;
        for part in &parts[1..] {
            match current {
                JsonValue::Object(map) => {
                    current = map.get(*part)?;
                }
                JsonValue::Array(arr) => {
                    let idx: usize = part.parse().ok()?;
                    current = arr.get(idx)?;
                }
                _ => return None,
            }
        }
        Some(current.clone())
    }

    // Internal inspect without borrowing self
    fn inspect_impl(
        &self,
        name: &str,
        vars: &HashMap<String, JsonValue>,
    ) -> Option<InspectionResult> {
        let parts: Vec<&str> = name.split('.').collect();
        let root_name = parts[0];

        if let Some(value) = vars.get(root_name) {
            let mut current = value.clone();
            for part in parts.iter().skip(1) {
                current = match current {
                    JsonValue::Object(ref obj) => obj.get(*part)?.clone(),
                    JsonValue::Array(ref arr) => {
                        let idx: usize = part.parse().ok()?;
                        arr.get(idx)?.clone()
                    }
                    _ => return None,
                };
            }
            Some(InspectionResult::found(name, current))
        } else {
            Some(InspectionResult::not_found(name))
        }
    }

    /// Record an inspection in history
    pub fn record(&mut self, result: InspectionResult) {
        self.history.push(result);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Get inspection history
    pub fn history(&self) -> &[InspectionResult] {
        &self.history
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    /// Search for variables matching a pattern
    pub fn search(
        &self,
        pattern: &str,
        vars: &HashMap<String, JsonValue>,
    ) -> Vec<InspectionResult> {
        let regex = match regex::Regex::new(pattern) {
            Ok(r) => r,
            Err(_) => return Vec::new(),
        };

        vars.iter()
            .filter(|(name, _)| regex.is_match(name))
            .map(|(name, value)| InspectionResult::found(name, value.clone()))
            .collect()
    }

    /// Get a diff between two variable snapshots
    pub fn diff(
        &self,
        before: &HashMap<String, JsonValue>,
        after: &HashMap<String, JsonValue>,
    ) -> VariableDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        // Find added and changed
        for (name, value) in after {
            match before.get(name) {
                Some(prev) if prev != value => {
                    changed.push(VariableChange {
                        name: name.clone(),
                        before: prev.clone(),
                        after: value.clone(),
                    });
                }
                None => {
                    added.push(InspectionResult::found(name, value.clone()));
                }
                _ => {}
            }
        }

        // Find removed
        for (name, value) in before {
            if !after.contains_key(name) {
                removed.push(InspectionResult::found(name, value.clone()));
            }
        }

        VariableDiff {
            added,
            removed,
            changed,
        }
    }
}

impl Default for VariableInspector {
    fn default() -> Self {
        Self::new()
    }
}

/// A change in a variable value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableChange {
    /// Variable name
    pub name: String,
    /// Value before change
    pub before: JsonValue,
    /// Value after change
    pub after: JsonValue,
}

/// Diff between two variable snapshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDiff {
    /// Variables that were added
    pub added: Vec<InspectionResult>,
    /// Variables that were removed
    pub removed: Vec<InspectionResult>,
    /// Variables that changed
    pub changed: Vec<VariableChange>,
}

impl VariableDiff {
    /// Check if there are any changes
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Get total number of changes
    pub fn count(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vars() -> HashMap<String, JsonValue> {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), JsonValue::String("test".to_string()));
        vars.insert("count".to_string(), serde_json::json!(42));
        vars.insert("items".to_string(), serde_json::json!(["a", "b", "c"]));
        vars.insert(
            "config".to_string(),
            serde_json::json!({"host": "localhost", "port": 8080}),
        );
        vars
    }

    #[test]
    fn test_inspect_simple_variable() {
        let inspector = VariableInspector::new();
        let vars = test_vars();

        let result = inspector.inspect("name", &vars).unwrap();
        assert!(result.found);
        assert_eq!(result.value, JsonValue::String("test".to_string()));
        assert_eq!(result.value_type, "string");
    }

    #[test]
    fn test_inspect_nested_variable() {
        let inspector = VariableInspector::new();
        let vars = test_vars();

        let result = inspector.inspect("config.host", &vars).unwrap();
        assert!(result.found);
        assert_eq!(result.value, JsonValue::String("localhost".to_string()));
    }

    #[test]
    fn test_inspect_array_index() {
        let inspector = VariableInspector::new();
        let vars = test_vars();

        let result = inspector.inspect("items.1", &vars).unwrap();
        assert!(result.found);
        assert_eq!(result.value, JsonValue::String("b".to_string()));
    }

    #[test]
    fn test_inspect_not_found() {
        let inspector = VariableInspector::new();
        let vars = test_vars();

        let result = inspector.inspect("nonexistent", &vars).unwrap();
        assert!(!result.found);
    }

    #[test]
    fn test_variable_watch() {
        let mut watch = VariableWatch::new("test");

        let value1 = serde_json::json!(1);
        let value2 = serde_json::json!(2);

        assert!(watch.update(&value1));
        assert_eq!(watch.change_count, 1);

        assert!(!watch.update(&value1)); // Same value
        assert_eq!(watch.change_count, 1);

        assert!(watch.update(&value2)); // Changed
        assert_eq!(watch.change_count, 2);
    }

    #[test]
    fn test_watch_condition() {
        let cond = WatchCondition::GreaterThan(10.0);
        assert!(cond.evaluate(&serde_json::json!(15)));
        assert!(!cond.evaluate(&serde_json::json!(5)));

        let cond = WatchCondition::Contains("foo".to_string());
        assert!(cond.evaluate(&serde_json::json!("foobar")));
        assert!(!cond.evaluate(&serde_json::json!("bar")));
    }

    #[test]
    fn test_inspector_watches() {
        let mut inspector = VariableInspector::new();
        let vars = test_vars();

        inspector.add_watch("name", None);
        inspector.add_watch("count", Some(VariableScope::Task));

        let results = inspector.inspect_watched(&vars);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_variable_diff() {
        let inspector = VariableInspector::new();

        let before: HashMap<String, JsonValue> = [
            ("a".to_string(), serde_json::json!(1)),
            ("b".to_string(), serde_json::json!(2)),
            ("c".to_string(), serde_json::json!(3)),
        ]
        .into_iter()
        .collect();

        let after: HashMap<String, JsonValue> = [
            ("a".to_string(), serde_json::json!(1)),  // Unchanged
            ("b".to_string(), serde_json::json!(99)), // Changed
            ("d".to_string(), serde_json::json!(4)),  // Added
        ]
        .into_iter()
        .collect();

        let diff = inspector.diff(&before, &after);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.changed.len(), 1);
        assert!(!diff.is_empty());
    }

    #[test]
    fn test_inspection_result_types() {
        let result = InspectionResult::found("arr", serde_json::json!([1, 2, 3]));
        assert_eq!(result.value_type, "list");
        assert_eq!(result.size, Some(3));

        let result = InspectionResult::found("obj", serde_json::json!({"a": 1}));
        assert_eq!(result.value_type, "dict");
        assert!(result.children.is_some());
    }
}
