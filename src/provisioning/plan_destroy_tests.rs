//! Destroy-order checks use only synthetic state and the actual plan builder.

use super::*;
use crate::provisioning::state::ResourceState;
use serde_json::json;

fn id(name: &str) -> ResourceId {
    ResourceId::new("fixture_item", name)
}

fn state(edges: &[(&str, &[&str])]) -> ProvisioningState {
    let mut state = ProvisioningState::new();
    for (name, dependencies) in edges {
        let mut resource = ResourceState::new(id(name), *name, "fixture", json!({}), json!({}));
        resource.dependencies = dependencies.iter().map(|name| id(name)).collect();
        state.add_resource(resource);
    }
    state
}

fn assert_before(plan: &ExecutionPlan, first: &str, second: &str) {
    let order: Vec<_> = plan
        .execution_order()
        .unwrap()
        .iter()
        .map(|a| a.resource_id.name.as_str())
        .collect();
    assert!(
        order.iter().position(|name| *name == first).unwrap()
            < order.iter().position(|name| *name == second).unwrap(),
        "{first} must precede {second}: {order:?}"
    );
}

#[test]
fn chain_destroy_and_config_removal_use_stored_dependencies() {
    for destroy_only in [false, true] {
        let builder = PlanBuilder::new(state(&[
            ("network", &[]),
            ("subnet", &["network"]),
            ("instance", &["subnet"]),
        ]));
        let mut plan = if destroy_only {
            builder.destroy()
        } else {
            builder
        }
        .build()
        .unwrap();
        // A caller's action-vector order must not change the dependency order.
        for reverse in [false, true] {
            plan.actions.sort_by_key(|a| a.resource_id.address());
            if reverse {
                plan.actions.reverse();
            }
            assert_before(&plan, "instance", "subnet");
            assert_before(&plan, "subnet", "network");
        }
    }
}

#[test]
fn diamond_destroy_preserves_both_dependency_paths() {
    let mut plan = PlanBuilder::new(state(&[
        ("base", &[]),
        ("left", &["base"]),
        ("right", &["base"]),
        ("leaf", &["left", "right"]),
    ]))
    .destroy()
    .build()
    .unwrap();
    for reverse in [false, true] {
        plan.actions.sort_by_key(|a| a.resource_id.address());
        if reverse {
            plan.actions.reverse();
        }
        for (first, second) in [
            ("leaf", "left"),
            ("leaf", "right"),
            ("left", "base"),
            ("right", "base"),
        ] {
            assert_before(&plan, first, second);
        }
    }
}

#[test]
fn selected_destroy_edges_do_not_expand_targets() {
    let plan = PlanBuilder::new(state(&[
        ("base", &[]),
        ("middle", &["base"]),
        ("outside", &["middle"]),
    ]))
    .destroy()
    .with_targets(vec![id("base"), id("middle")])
    .build()
    .unwrap();
    assert_eq!(plan.actions.len(), 2);
    assert_eq!(plan.to_destroy.len(), 2);
    assert!(plan
        .actions
        .iter()
        .all(|action| action.resource_id != id("outside")
            && !action.depends_on.contains(&id("outside"))));
    assert_before(&plan, "middle", "base");
}

#[test]
fn cyclic_stored_destroy_dependencies_report_an_error() {
    let plan = PlanBuilder::new(state(&[("first", &["second"]), ("second", &["first"])]))
        .destroy()
        .build()
        .unwrap();
    assert!(matches!(
        plan.execution_order(),
        Err(ProvisioningError::DependencyCycle(_))
    ));
}

#[test]
fn explicit_dependency_override_is_respected_for_destroy() {
    let mut plan = PlanBuilder::new(state(&[("first", &["second"]), ("second", &[])]))
        .destroy()
        .with_dependencies(id("first"), vec![])
        .with_dependencies(id("second"), vec![id("first")])
        .build()
        .unwrap();
    for reverse in [false, true] {
        plan.actions.sort_by_key(|a| a.resource_id.address());
        if reverse {
            plan.actions.reverse();
        }
        assert_before(&plan, "second", "first");
    }
}

#[test]
fn create_dependencies_keep_prerequisites_first() {
    let mut plan = PlanBuilder::new(ProvisioningState::new())
        .with_resource(id("base"), json!({}))
        .with_resource(id("leaf"), json!({}))
        .with_dependencies(id("leaf"), vec![id("base")])
        .build()
        .unwrap();
    for reverse in [false, true] {
        plan.actions.sort_by_key(|a| a.resource_id.address());
        if reverse {
            plan.actions.reverse();
        }
        assert_before(&plan, "base", "leaf");
    }
}
