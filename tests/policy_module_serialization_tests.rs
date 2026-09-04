//! Actual production serializers only; no task or handler is executed.

use rustible::executor::task::{BlockRole, Handler, Task};
use rustible::policy::pack::{PackRule, RuleCheck};
use rustible::policy::{BuiltinRule, PolicyEngine, PolicySet, RuleCondition, RuleSeverity};
use serde_json::{json, Value};

fn inspect(input: Value, allowed: bool) {
    let builtin = PolicySet {
        engine: PolicyEngine::Builtin,
        opa_policy_path: None,
        builtin_rules: vec![BuiltinRule {
            name: "no-shell".into(),
            description: "Serialization fixture".into(),
            severity: RuleSeverity::Error,
            condition: RuleCondition::DenyModule {
                module_name: "shell".into(),
            },
        }],
    };
    let pack = PackRule {
        name: "no-shell".into(),
        description: "Serialization fixture".into(),
        severity: RuleSeverity::Error,
        check: RuleCheck::ModuleBlacklist(vec!["shell".into()]),
    };
    let decision = builtin.evaluate(&input);
    let violations = pack.evaluate(&input);
    assert!(
        decision.is_ok(),
        "serialized production input: {decision:?}; pack: {violations:?}"
    );
    assert_eq!(decision.unwrap().allowed, allowed);
    assert_eq!(violations.is_empty(), allowed);
}

#[test]
fn actual_executor_task_serialization_preserves_module_checks() {
    for (module, allowed) in [("debug", true), ("ansible.builtin.shell", false)] {
        let task = Task {
            name: "serialized fixture".into(),
            module: module.into(),
            loop_var: "fixture_item".into(),
            block_id: Some("fixture_block".into()),
            block_role: BlockRole::Rescue,
            ..Task::default()
        };
        let serialized = serde_json::to_value(task).unwrap();
        assert!(serialized.get("loop_var").is_some());
        assert!(serialized.get("block_id").is_some());
        assert!(serialized.get("block_role").is_some());
        inspect(json!({"tasks": [serialized]}), allowed);
    }
}

#[test]
fn actual_executor_handler_serialization_preserves_module_checks() {
    for (module, allowed) in [("debug", true), ("ansible.legacy.shell", false)] {
        let handler = Handler {
            name: "serialized fixture".into(),
            module: module.into(),
            args: Default::default(),
            when: Some("fixture_condition".into()),
            listen: vec!["fixture_topic".into()],
        };
        inspect(
            json!({"handlers": [serde_json::to_value(handler).unwrap()]}),
            allowed,
        );
    }
}
