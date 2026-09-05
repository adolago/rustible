//! Input-only graph regressions. No commands, network, or managed-host effects.
//! Baseline cycle regressions must run in a bounded child: stack overflow aborts
//! the process and cannot be contained by catch_unwind.

use rustible::executor::runtime::{InventoryGroup, RuntimeContext};
use rustible::inventory::{Group, Host, Inventory, InventoryError};
use tempfile::tempdir;

fn load(extension: &str, content: &str) -> Result<Inventory, InventoryError> {
    let directory = tempdir().unwrap();
    let path = directory.path().join(format!("hosts.{extension}"));
    std::fs::write(&path, content).unwrap();
    Inventory::load(path)
}

#[test]
fn cyclic_group_files_are_rejected_in_each_format() {
    for (extension, content) in [
        ("ini", "[a:children]\na\n"),
        ("ini", "[a:children]\nb\n[b:children]\na\n"),
        ("yml", "a: {children: {a: null}}"),
        ("yml", "a: {children: {b: {children: {a: null}}}}"),
        ("json", r#"{"a":{"children":["a"]}}"#),
        ("json", r#"{"a":{"children":["b"]},"b":{"children":["a"]}}"#),
    ] {
        assert!(
            matches!(
                load(extension, content),
                Err(InventoryError::CircularDependency(_))
            ),
            "accepted a cyclic {extension} inventory"
        );
    }
}

#[test]
fn cycles_in_inventory_directory_hosts_file_are_rejected() {
    let directory = tempdir().unwrap();
    std::fs::write(
        directory.path().join("hosts.ini"),
        "[a:children]\nb\n[b:children]\na\n",
    )
    .unwrap();
    assert!(matches!(
        Inventory::load(directory.path()),
        Err(InventoryError::CircularDependency(_))
    ));
}

#[test]
fn adding_cyclic_group_is_rejected_without_changing_inventory() {
    let mut inventory = Inventory::new();
    let mut first = Group::new("a");
    first.add_child("b");
    inventory.add_group(first).unwrap();
    let mut second = Group::new("b");
    second.add_child("a");
    assert!(matches!(
        inventory.add_group(second),
        Err(InventoryError::CircularDependency(_))
    ));
    assert!(inventory.get_group("b").is_none());
    assert!(inventory.get_group("a").unwrap().parents.is_empty());
    inventory.add_group(Group::new("b")).unwrap();
    assert!(inventory.get_group("b").unwrap().parents.contains("a"));
}

#[test]
fn replacing_group_rolls_back_on_cycle_and_clears_removed_parent_edges() {
    let mut inventory = Inventory::new();
    let mut first = Group::new("a");
    first.add_child("b");
    inventory.add_group(first).unwrap();
    inventory.add_group(Group::new("b")).unwrap();
    let mut cyclic = Group::new("b");
    cyclic.add_child("a");
    assert!(inventory.add_group(cyclic).is_err());
    assert!(inventory.get_group("b").unwrap().children.is_empty());
    assert!(inventory.get_group("b").unwrap().parents.contains("a"));
    inventory.add_group(Group::new("a")).unwrap();
    assert!(inventory.get_group("b").unwrap().parents.is_empty());
}

#[test]
fn mutation_cannot_make_inventory_pattern_traversal_recurse_forever() {
    let mut inventory = Inventory::new();
    inventory.add_group(Group::new("a")).unwrap();
    inventory.get_group_mut("a").unwrap().add_child("a");
    assert!(matches!(
        inventory.get_hosts_for_pattern("a"),
        Err(InventoryError::CircularDependency(_))
    ));
}

#[test]
fn changing_display_name_does_not_hide_a_distinct_child_group() {
    let mut inventory = Inventory::new();
    inventory.add_host(Host::new("node")).unwrap();
    let mut child = Group::new("b");
    child.add_host("node");
    inventory.add_group(child).unwrap();
    let mut parent = Group::new("a");
    parent.add_child("b");
    inventory.add_group(parent).unwrap();
    inventory.get_group_mut("b").unwrap().name = "a".into();
    let hosts = inventory.get_hosts_for_pattern("a").unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].name, "node");
}

#[test]
fn acyclic_diamond_deduplicates_hosts_and_preserves_variable_inheritance() {
    let inventory = load("ini", "[top:children]\nleft\nright\n[left:children]\nleaf\n[right:children]\nleaf\n[leaf]\nnode\n[top:vars]\nparent=present\n[leaf:vars]\nchild=present\n").unwrap();
    let hosts = inventory.get_hosts_for_pattern("top").unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].name, "node");
    let vars = inventory.get_host_vars(hosts[0]);
    assert_eq!(vars["parent"].as_str(), Some("present"));
    assert_eq!(vars["child"].as_str(), Some("present"));
}

#[test]
fn deep_acyclic_group_chain_does_not_require_recursive_host_traversal() {
    let mut content = String::new();
    for index in 0..4096 {
        content.push_str(&format!("[group{index}:children]\ngroup{}\n", index + 1));
    }
    content.push_str("[group4096]\nnode\n");
    let inventory = load("ini", &content).unwrap();
    assert_eq!(
        inventory.get_hosts_for_pattern("group0").unwrap()[0].name,
        "node"
    );
}

#[test]
fn deep_group_variable_inheritance_preserves_root_value() {
    let mut content = String::new();
    for index in 0..4096 {
        content.push_str(&format!("[group{index}:children]\ngroup{}\n", index + 1));
    }
    content.push_str("[group4096]\nnode\n[group0:vars]\nroot_value=present\n");
    let inventory = load("ini", &content).unwrap();
    let vars = inventory.get_host_vars(inventory.get_host("node").unwrap());
    assert_eq!(vars["root_value"].as_str(), Some("present"));
}

#[test]
fn runtime_direct_cycles_terminate_and_deduplicate_hosts() {
    let mut context = RuntimeContext::new();
    context.add_group(
        "a".into(),
        InventoryGroup {
            hosts: vec!["first".into(), "first".into()],
            children: vec!["b".into(), "a".into()],
            ..Default::default()
        },
    );
    context.add_group(
        "b".into(),
        InventoryGroup {
            hosts: vec!["second".into(), "first".into()],
            children: vec!["a".into()],
            ..Default::default()
        },
    );
    assert_eq!(
        context.get_group_hosts("a"),
        Some(vec!["first".into(), "second".into()])
    );
    assert_eq!(context.get_group_hosts("missing"), None);
}

#[test]
fn runtime_diamond_preserves_depth_first_host_order() {
    let mut context = RuntimeContext::new();
    for (name, hosts, children) in [
        ("top", vec!["top-host"], vec!["left", "right"]),
        ("left", vec!["left-host"], vec!["leaf"]),
        ("right", vec!["right-host"], vec!["leaf", "missing"]),
        ("leaf", vec!["leaf-host"], vec![]),
    ] {
        context.add_group(
            name.into(),
            InventoryGroup {
                hosts: hosts.into_iter().map(String::from).collect(),
                children: children.into_iter().map(String::from).collect(),
                ..Default::default()
            },
        );
    }
    assert_eq!(
        context.get_group_hosts("top").unwrap(),
        ["top-host", "left-host", "leaf-host", "right-host"]
    );
}

#[test]
fn missing_forward_group_reference_remains_supported() {
    let mut inventory = Inventory::new();
    let mut group = Group::new("parent");
    group.add_child("future");
    group.add_host("node");
    inventory.add_host(Host::new("node")).unwrap();
    inventory.add_group(group).unwrap();
    assert_eq!(inventory.get_hosts_for_pattern("parent").unwrap().len(), 1);
}

#[test]
fn all_three_group_graphs_match_an_independent_cycle_oracle() {
    for mask in 0..512_u16 {
        let mut groups = serde_json::Map::new();
        let mut indegree = [0; 3];
        for parent in 0..3 {
            let mut children = Vec::new();
            for (child, degree) in indegree.iter_mut().enumerate() {
                if mask & (1 << (parent * 3 + child)) != 0 {
                    children.push(format!("g{child}"));
                    *degree += 1;
                }
            }
            groups.insert(
                format!("g{parent}"),
                serde_json::json!({"children": children}),
            );
        }
        // Independent topological-removal oracle, not production's DFS algorithm.
        let mut removed = [false; 3];
        while let Some(parent) = (0..3).find(|&group| !removed[group] && indegree[group] == 0) {
            removed[parent] = true;
            for (child, degree) in indegree.iter_mut().enumerate() {
                if mask & (1 << (parent * 3 + child)) != 0 {
                    *degree -= 1;
                }
            }
        }
        let expected_cycle = removed.contains(&false);
        let result = load("json", &serde_json::to_string(&groups).unwrap());
        assert_eq!(
            matches!(result, Err(InventoryError::CircularDependency(_))),
            expected_cycle,
            "graph mask {mask}"
        );
        if !expected_cycle {
            assert!(result.is_ok());
        }
    }
}
