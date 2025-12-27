//! Condition evaluation for changed_when/failed_when patterns.
//!
//! This module provides condition evaluation capabilities for determining
//! whether tasks have changed state or failed based on their output.

use indexmap::IndexMap;
use serde_json::Value as JsonValue;

/// A condition that can be evaluated against execution context.
///
/// Conditions are used for `when`, `changed_when`, and `failed_when` clauses
/// in task definitions.
#[derive(Debug, Clone)]
pub enum Condition {
    /// Always evaluates to true
    Always,
    /// Always evaluates to false
    Never,
    /// A boolean literal
    Boolean(bool),
    /// A Jinja2-like expression to evaluate
    Expression(String),
}

impl Condition {
    /// Create a condition from a string expression
    pub fn from_expression(expr: impl Into<String>) -> Self {
        Condition::Expression(expr.into())
    }

    /// Create an always-true condition
    pub fn always() -> Self {
        Condition::Always
    }

    /// Create an always-false condition
    pub fn never() -> Self {
        Condition::Never
    }

    /// Create a boolean condition
    pub fn boolean(value: bool) -> Self {
        Condition::Boolean(value)
    }
}

impl Default for Condition {
    fn default() -> Self {
        Condition::Always
    }
}

/// Context for condition evaluation.
///
/// Provides access to variables and task results needed to evaluate conditions.
#[derive(Debug, Clone, Default)]
pub struct ConditionContext {
    /// Variables available during evaluation
    pub variables: IndexMap<String, JsonValue>,
    /// The result of the current task (if available)
    pub task_result: Option<TaskResultContext>,
}

/// Task result context for condition evaluation
#[derive(Debug, Clone, Default)]
pub struct TaskResultContext {
    /// Return code of the command (if applicable)
    pub rc: Option<i32>,
    /// Standard output
    pub stdout: Option<String>,
    /// Standard error
    pub stderr: Option<String>,
    /// Whether the task reported a change
    pub changed: bool,
    /// Whether the task failed
    pub failed: bool,
}

impl ConditionContext {
    /// Create a new empty condition context
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with variables
    pub fn with_variables(variables: IndexMap<String, JsonValue>) -> Self {
        Self {
            variables,
            task_result: None,
        }
    }

    /// Set task result context
    pub fn with_task_result(mut self, result: TaskResultContext) -> Self {
        self.task_result = Some(result);
        self
    }

    /// Get a variable by name
    pub fn get_variable(&self, name: &str) -> Option<&JsonValue> {
        self.variables.get(name)
    }

    /// Check if a variable is defined
    pub fn is_defined(&self, name: &str) -> bool {
        self.variables.contains_key(name)
    }
}

/// Evaluator for condition expressions.
///
/// Provides methods to evaluate conditions against a context.
#[derive(Debug, Default)]
pub struct ConditionEvaluator {
    /// Enable strict mode (fail on undefined variables)
    pub strict_mode: bool,
}

impl ConditionEvaluator {
    /// Create a new condition evaluator
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an evaluator with strict mode enabled
    pub fn strict() -> Self {
        Self { strict_mode: true }
    }

    /// Evaluate a condition against the given context
    pub fn evaluate(&self, condition: &Condition, ctx: &ConditionContext) -> Result<bool, String> {
        match condition {
            Condition::Always => Ok(true),
            Condition::Never => Ok(false),
            Condition::Boolean(b) => Ok(*b),
            Condition::Expression(expr) => self.evaluate_expression(expr, ctx),
        }
    }

    /// Evaluate a string expression
    fn evaluate_expression(&self, expr: &str, ctx: &ConditionContext) -> Result<bool, String> {
        let expr = expr.trim();

        // Handle empty expression
        if expr.is_empty() {
            return Ok(true);
        }

        // Handle simple boolean literals
        match expr.to_lowercase().as_str() {
            "true" | "yes" => return Ok(true),
            "false" | "no" => return Ok(false),
            _ => {}
        }

        // Handle variable references
        if let Some(value) = ctx.get_variable(expr) {
            return Ok(is_truthy(value));
        }

        // Handle defined() check
        if let Some(inner) = expr.strip_prefix("defined(").and_then(|s| s.strip_suffix(')')) {
            return Ok(ctx.is_defined(inner.trim()));
        }

        // Handle undefined() check
        if let Some(inner) = expr
            .strip_prefix("undefined(")
            .and_then(|s| s.strip_suffix(')'))
        {
            return Ok(!ctx.is_defined(inner.trim()));
        }

        // Handle 'not' prefix
        if let Some(inner) = expr.strip_prefix("not ") {
            return self.evaluate_expression(inner.trim(), ctx).map(|v| !v);
        }

        // Handle simple comparisons with 'rc'
        if let Some(result) = &ctx.task_result {
            if let Some(rc) = result.rc {
                // Pattern: rc == N or rc != N
                if let Some(rest) = expr.strip_prefix("rc") {
                    let rest = rest.trim();
                    if let Some(num_str) = rest.strip_prefix("==") {
                        if let Ok(n) = num_str.trim().parse::<i32>() {
                            return Ok(rc == n);
                        }
                    } else if let Some(num_str) = rest.strip_prefix("!=") {
                        if let Ok(n) = num_str.trim().parse::<i32>() {
                            return Ok(rc != n);
                        }
                    }
                }
            }
        }

        // Default: if in strict mode, fail on unknown expressions
        if self.strict_mode {
            Err(format!("Unable to evaluate expression: {}", expr))
        } else {
            // Non-strict: treat unknown as false
            Ok(false)
        }
    }
}

/// Check if a JSON value is truthy
fn is_truthy(value: &JsonValue) -> bool {
    match value {
        JsonValue::Null => false,
        JsonValue::Bool(b) => *b,
        JsonValue::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        JsonValue::String(s) => !s.is_empty() && s.to_lowercase() != "false" && s != "0",
        JsonValue::Array(a) => !a.is_empty(),
        JsonValue::Object(o) => !o.is_empty(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_condition_always() {
        let eval = ConditionEvaluator::new();
        let ctx = ConditionContext::new();
        assert!(eval.evaluate(&Condition::Always, &ctx).unwrap());
    }

    #[test]
    fn test_condition_never() {
        let eval = ConditionEvaluator::new();
        let ctx = ConditionContext::new();
        assert!(!eval.evaluate(&Condition::Never, &ctx).unwrap());
    }

    #[test]
    fn test_condition_boolean() {
        let eval = ConditionEvaluator::new();
        let ctx = ConditionContext::new();
        assert!(eval.evaluate(&Condition::Boolean(true), &ctx).unwrap());
        assert!(!eval.evaluate(&Condition::Boolean(false), &ctx).unwrap());
    }

    #[test]
    fn test_expression_literals() {
        let eval = ConditionEvaluator::new();
        let ctx = ConditionContext::new();

        assert!(eval
            .evaluate(&Condition::Expression("true".into()), &ctx)
            .unwrap());
        assert!(!eval
            .evaluate(&Condition::Expression("false".into()), &ctx)
            .unwrap());
    }

    #[test]
    fn test_defined_check() {
        let eval = ConditionEvaluator::new();
        let mut vars = IndexMap::new();
        vars.insert("my_var".into(), JsonValue::String("value".into()));
        let ctx = ConditionContext::with_variables(vars);

        assert!(eval
            .evaluate(&Condition::Expression("defined(my_var)".into()), &ctx)
            .unwrap());
        assert!(!eval
            .evaluate(&Condition::Expression("defined(other_var)".into()), &ctx)
            .unwrap());
    }

    #[test]
    fn test_not_expression() {
        let eval = ConditionEvaluator::new();
        let ctx = ConditionContext::new();

        assert!(!eval
            .evaluate(&Condition::Expression("not true".into()), &ctx)
            .unwrap());
        assert!(eval
            .evaluate(&Condition::Expression("not false".into()), &ctx)
            .unwrap());
    }

    #[test]
    fn test_is_truthy() {
        assert!(!is_truthy(&JsonValue::Null));
        assert!(!is_truthy(&JsonValue::Bool(false)));
        assert!(is_truthy(&JsonValue::Bool(true)));
        assert!(!is_truthy(&JsonValue::String("".into())));
        assert!(is_truthy(&JsonValue::String("hello".into())));
        assert!(!is_truthy(&JsonValue::Array(vec![])));
        assert!(is_truthy(&JsonValue::Array(vec![JsonValue::Null])));
    }
}
