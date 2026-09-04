//! Policy evaluation only: these fixtures never parse or execute a task command.

use rustible::policy::pack::{PackRule, RuleCheck};
use rustible::policy::{BuiltinRule, PolicyEngine, PolicySet, RuleCondition, RuleSeverity};
use serde_json::{json, Value};

fn policies(module: &str) -> (PolicySet, PackRule) {
    (
        PolicySet {
            engine: PolicyEngine::Builtin,
            opa_policy_path: None,
            builtin_rules: vec![BuiltinRule {
                name: "deny-fixture-module".into(),
                description: "Pure policy fixture".into(),
                severity: RuleSeverity::Error,
                condition: RuleCondition::DenyModule {
                    module_name: module.into(),
                },
            }],
        },
        PackRule {
            name: "deny-fixture-module".into(),
            description: "Pure policy fixture".into(),
            severity: RuleSeverity::Error,
            check: RuleCheck::ModuleBlacklist(vec![module.into()]),
        },
    )
}

fn assert_denied(input: Value, module: &str) {
    let (builtin, pack) = policies(module);
    let decision = builtin.evaluate(&input).expect("well-formed policy input");
    let violations = pack.evaluate(&input);
    assert!(
        !decision.allowed
            && decision.reasons.iter().any(|r| r.contains("denied module"))
            && violations.iter().any(|r| r.contains("denied module")),
        "module {module}: builtin={decision:?}, pack={violations:?}"
    );
}

fn assert_allowed(input: Value, module: &str) {
    let (builtin, pack) = policies(module);
    assert!(builtin.evaluate(&input).unwrap().allowed);
    assert!(pack.evaluate(&input).is_empty());
}

fn assert_invalid(input: Value) {
    let (builtin, pack) = policies("shell");
    let decision = builtin.evaluate(&input);
    let violations = pack.evaluate(&input);
    assert!(
        decision
            .as_ref()
            .is_err_and(|error| error.to_string().contains("Invalid policy input"))
            && violations
                .iter()
                .any(|r| r.contains("Invalid policy input")),
        "invalid input: builtin={decision:?}, pack={violations:?}"
    );
}

#[test]
fn denies_direct_tasks() {
    assert_denied(
        json!([{"tasks": [{"name": "fixture", "shell": {}}]}]),
        "shell",
    );
}

#[test]
fn denies_pre_tasks() {
    assert_denied(json!([{"pre_tasks": [{"shell": {}}]}]), "shell");
}

#[test]
fn denies_post_tasks() {
    assert_denied(json!({"plays": [{"post_tasks": [{"shell": {}}]}]}), "shell");
}

#[test]
fn denies_handlers() {
    assert_denied(
        json!({"handlers": [{"name": "fixture", "shell": {}}]}),
        "shell",
    );
}

#[test]
fn denies_block_tasks() {
    assert_denied(json!([{"tasks": [{"block": [{"shell": {}}]}]}]), "shell");
}

#[test]
fn denies_rescue_tasks() {
    assert_denied(
        json!([{"tasks": [{"block": [], "rescue": [{"shell": {}}]}]}]),
        "shell",
    );
}

#[test]
fn denies_always_tasks() {
    assert_denied(
        json!([{"tasks": [{"block": [], "always": [{"shell": {}}]}]}]),
        "shell",
    );
}

#[test]
fn denies_nested_containers() {
    assert_denied(
        json!({"pre_tasks": [{"block": [{"rescue": [{"always": [{"shell": {}}]}]}]}]}),
        "shell",
    );
}

#[test]
fn normalizes_supported_module_aliases() {
    for module in [
        "ansible.builtin.shell",
        "ansible.legacy.shell",
        "ansible.builtin.extra.shell",
    ] {
        assert_denied(json!({"tasks": [{(module): {}}]}), "shell");
    }
}

#[test]
fn normalizes_policy_module_names() {
    for module in ["ansible.builtin.shell", "ansible.legacy.shell"] {
        assert_denied(json!({"tasks": [{"shell": {}}]}), module);
    }
}

#[test]
fn inspects_serialized_executor_module_names() {
    for container in ["tasks", "handlers"] {
        assert_denied(
            json!({(container): [{"name": "fixture", "module": "ansible.builtin.shell", "args": {}}]}),
            "shell",
        );
    }
}

#[test]
fn rejects_lossy_serialized_public_tasks() {
    assert_invalid(json!({"tasks": [{"name": "fixture", "module": {"args": {}}}]}));
    assert_invalid(json!({"handlers": [{"name": "fixture", "task": {"module": {"args": {}}}}]}));
    assert_invalid(json!({"handlers": [{"name": "fixture", "task": null}]}));
    assert_denied(
        json!({"handlers": [{"name": "fixture", "task": {"module": "ansible.legacy.shell", "args": {}}}]}),
        "shell",
    );
}

#[test]
fn rejects_invalid_module_identity() {
    for module in [json!(null), json!(42), json!(""), json!(" ")] {
        assert_invalid(json!({"tasks": [{"module": module}]}));
    }
}

#[test]
fn does_not_alias_third_party_module_names() {
    assert_allowed(json!({"tasks": [{"community.general.shell": {}}]}), "shell");
    assert_denied(
        json!({"tasks": [{"community.general.shell": {}}]}),
        "community.general.shell",
    );
}

#[test]
fn does_not_traverse_argument_or_variable_data() {
    assert_allowed(
        json!({
            "vars": {"pre_tasks": [{"shell": {}}]},
            "tasks": [{"debug": {"msg": {"block": [{"shell": {}}]}}, "vars": {"shell": {}}}]
        }),
        "shell",
    );
}

#[test]
fn rejects_malformed_playbook_shapes() {
    for input in [
        json!(null),
        json!(42),
        json!("fixture"),
        json!([false]),
        json!({"plays": {}}),
        json!({"plays": null}),
    ] {
        assert_invalid(input);
    }
}

#[test]
fn rejects_malformed_task_containers() {
    for field in ["pre_tasks", "tasks", "post_tasks", "handlers"] {
        assert_invalid(json!({(field): {"shell": {}}}));
    }
    for field in ["block", "rescue", "always"] {
        assert_invalid(json!({"tasks": [{(field): {"shell": {}}}]}));
    }
}

#[test]
fn rejects_non_object_tasks() {
    for task in [json!(null), json!(42), json!("fixture"), json!([])] {
        assert_invalid(json!({"tasks": [task]}));
    }
}

#[test]
fn permits_empty_and_null_task_containers() {
    for input in [
        json!([]),
        json!({}),
        json!({"plays": []}),
        json!({"pre_tasks": null, "tasks": [], "post_tasks": null, "handlers": null}),
        json!({"tasks": [{"debug": {}, "block": null, "rescue": null, "always": null}]}),
    ] {
        assert_allowed(input, "shell");
    }
}

#[test]
fn bounds_task_nesting() {
    let mut task = json!({"shell": {}});
    for _ in 0..64 {
        task = json!({"block": [task]});
    }
    assert_denied(json!({"tasks": [task.clone()]}), "shell");
    assert_invalid(json!({"tasks": [{"block": [task]}]}));
}

#[test]
fn bounds_visited_nodes() {
    assert_invalid(json!({"tasks": vec![json!({"debug": {}}); 10_001]}));
    assert_allowed(json!({"tasks": vec![json!({"debug": {}}); 9_999]}), "shell");
    assert_invalid(json!({"tasks": [
        {"block": vec![json!({"debug": {}}); 9_998]},
        {"debug": {}}
    ]}));
}

#[test]
fn denies_implicit_debug_tasks() {
    assert_denied(json!({"tasks": [{"name": "implicit debug"}]}), "debug");
}

#[test]
fn rejects_ambiguous_task_modules() {
    assert_invalid(json!({"tasks": [{"debug": {}, "copy": {}}]}));
    assert_invalid(json!({"tasks": [{"module": "debug", "shell": {}}]}));
}

#[test]
fn rejects_unresolved_external_task_content() {
    assert_invalid(json!({"roles": ["fixture-role"]}));
    assert_invalid(json!({"import_playbook": "fixture.yml"}));
    for module in [
        "include_tasks",
        "import_tasks",
        "include_role",
        "import_role",
        "ansible.builtin.include_tasks",
        "ansible.legacy.import_role",
    ] {
        assert_invalid(json!({"tasks": [{(module): "fixture"}]}));
        assert_invalid(json!({"tasks": [{"module": module, "args": {}}]}));
    }
    assert_allowed(json!({"roles": [], "tasks": []}), "shell");
}

#[test]
fn rejects_executable_fields_outside_wrapped_handler() {
    for field in [
        "block",
        "rescue",
        "always",
        "include_tasks",
        "import_tasks",
        "include_role",
        "import_role",
    ] {
        assert_invalid(json!({"handlers": [{
            "name": "fixture",
            "task": {"module": "debug", "args": {}},
            (field): [{"shell": {}}]
        }]}));
    }
}
