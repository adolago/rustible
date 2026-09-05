//! Safe input-only regressions: no commands, network, credentials, or managed-host effects.
//! Files are temporary, non-executable inventory fixtures. Run this target with a timeout.

use rustible::inventory::{Host, Inventory};
use rustible::vars::{HashBehaviour, VarPrecedence, VarStore};
use serde_yaml::Value;
use tempfile::tempdir;

fn inventory(
    extension: &str,
    content: &str,
) -> Result<Inventory, rustible::inventory::InventoryError> {
    let directory = tempdir().unwrap();
    let path = directory.path().join(format!("hosts.{extension}"));
    std::fs::write(&path, content).unwrap();
    Inventory::load(path)
}

fn yaml(text: &str) -> Value {
    serde_yaml::from_str(text).unwrap()
}

#[test]
fn ini_group_scalars_preserve_fraction_and_unsigned_range() {
    let inv = inventory("ini", "[web:vars]\nratio=1.5\nlarge=18446744073709551615\n").unwrap();
    let group = inv.get_group("web").unwrap();
    assert_eq!(group.get_var("ratio").unwrap().as_f64(), Some(1.5));
    assert_eq!(group.get_var("large").unwrap().as_u64(), Some(u64::MAX));
}

#[test]
fn json_scalars_preserve_nested_numbers_and_types() {
    let inv = inventory("json", r#"{"web":{"hosts":["node"],"vars":{"ratio":1.5}},"_meta":{"hostvars":{"node":{"nested":[18446744073709551615,-2,0.125,true,null]}}}}"#).unwrap();
    assert_eq!(
        inv.get_group("web")
            .unwrap()
            .get_var("ratio")
            .unwrap()
            .as_f64(),
        Some(1.5)
    );
    assert_eq!(
        inv.get_host("node").unwrap().get_var("nested"),
        Some(&yaml("[18446744073709551615, -2, 0.125, true, null]"))
    );
}

#[test]
fn ini_unmatched_quotes_return_error_without_panicking() {
    for value in ["'", "\"", "'unterminated", "\"unterminated"] {
        let result =
            std::panic::catch_unwind(|| inventory("ini", &format!("[web:vars]\nlabel={value}\n")));
        assert!(result.is_ok(), "malformed INI value panicked");
        assert!(result.unwrap().is_err(), "malformed quote was accepted");
    }
}

#[test]
fn ini_valid_quoted_strings_and_booleans_are_preserved() {
    let inv = inventory(
        "ini",
        "[web:vars]\nlabel='two words'\nempty=\"\"\nnumber=\"1.5\"\nenabled=yes\n",
    )
    .unwrap();
    let group = inv.get_group("web").unwrap();
    assert_eq!(group.get_var("label").unwrap().as_str(), Some("two words"));
    assert_eq!(group.get_var("empty").unwrap().as_str(), Some(""));
    assert_eq!(group.get_var("number").unwrap().as_str(), Some("1.5"));
    assert_eq!(group.get_var("enabled").unwrap().as_bool(), Some(true));
}

#[test]
fn ini_hosts_keep_quoted_spaces_escapes_and_empty_values() {
    let host =
        Host::parse(r#"node label='two words' empty="" quote="a\"b" ansible_user="test user""#)
            .unwrap();
    assert_eq!(host.get_var("label").unwrap().as_str(), Some("two words"));
    assert_eq!(host.get_var("empty").unwrap().as_str(), Some(""));
    assert_eq!(host.get_var("quote").unwrap().as_str(), Some("a\"b"));
    assert_eq!(host.connection.ssh.user.as_deref(), Some("test user"));
}

#[test]
fn ini_hosts_reject_unterminated_quotes() {
    for line in [
        "node label='",
        "node label=\"unfinished",
        "node stray-token",
        "\"\"",
    ] {
        assert!(
            Host::parse(line).is_err(),
            "invalid host definition accepted"
        );
    }
}

#[test]
fn ini_comments_do_not_break_quoted_values() {
    let host = Host::parse("node label='literal # value' # ignored comment").unwrap();
    assert_eq!(
        host.get_var("label").unwrap().as_str(),
        Some("literal # value")
    );
    assert!(Host::parse("# only a comment").is_err());
}

#[test]
fn yaml_loads_all_and_sibling_groups() {
    let inv = inventory(
        "yml",
        "all:\n  hosts:\n    first:\nweb:\n  hosts:\n    second:\n",
    )
    .unwrap();
    assert_eq!(inv.host_count(), 2);
    assert!(inv.get_group("web").unwrap().has_host("second"));
    assert_eq!(inv.get_hosts_for_pattern("all").unwrap().len(), 2);
}

#[test]
fn yaml_rejects_wrong_inventory_shapes() {
    for content in [
        "[]",
        "scalar",
        "42",
        "all: []",
        "all: {hosts: [node]}",
        "all: {children: [web]}",
        "all: {vars: []}",
        "all: {hosts: {node: scalar}}",
        "1: {}",
        "all: {hosts: {1: {}}}",
        "all: {children: {1: {}}}",
        "all: {vars: {1: value}}",
        "all: {hosts: {node: {1: value}}}",
        "all: {host: {node: {}}}",
    ] {
        assert!(
            inventory("yml", content).is_err(),
            "malformed YAML structure accepted: {content}"
        );
    }
}

#[test]
fn yaml_accepts_empty_groups_null_hosts_and_nested_children() {
    let inv = inventory("yml", "empty:\nall:\n  children:\n    web:\n      hosts:\n        node:\n      vars:\n        label: example\n").unwrap();
    assert!(inv.get_group("empty").is_some());
    assert!(inv.get_host("node").is_some());
    assert_eq!(inv.get_hosts_for_pattern("web").unwrap().len(), 1);
}

#[test]
fn inventory_rejects_invalid_ports_in_every_format() {
    for port in ["0", "65536", "-1", "1.5", "true", "null", "[]"] {
        assert!(
            inventory(
                "yml",
                &format!("all: {{hosts: {{node: {{ansible_port: {port}}}}}}}")
            )
            .is_err(),
            "YAML port accepted: {port}"
        );
        assert!(inventory("json", &format!(r#"{{"all":{{"hosts":["node"]}},"_meta":{{"hostvars":{{"node":{{"ansible_port":{port}}}}}}}}}"#)).is_err(), "JSON port accepted: {port}");
    }
    for port in ["0", "65536", "-1", "invalid"] {
        assert!(Host::parse(&format!("node ansible_port={port}")).is_err());
    }
}

#[test]
fn inventory_accepts_port_boundaries_and_numeric_strings() {
    for port in [1_u16, 22, 65535] {
        for encoded in [port.to_string(), format!("\"{port}\"")] {
            let inv = inventory(
                "yml",
                &format!("all: {{hosts: {{node: {{ansible_port: {encoded}}}}}}}"),
            )
            .unwrap();
            assert_eq!(inv.get_host("node").unwrap().connection.ssh.port, port);
            let inv = inventory("json", &format!(r#"{{"all":{{"hosts":["node"]}},"_meta":{{"hostvars":{{"node":{{"ansible_port":{encoded}}}}}}}}}"#)).unwrap();
            assert_eq!(inv.get_host("node").unwrap().connection.ssh.port, port);
        }
    }
}

#[test]
fn inventory_rejects_invalid_group_ports() {
    for port in ["0", "65536", "-1", "1.5"] {
        assert!(inventory("yml", &format!("all: {{vars: {{ansible_port: {port}}}}}")).is_err());
        assert!(inventory(
            "json",
            &format!(r#"{{"all":{{"vars":{{"ansible_port":{port}}}}}}}"#)
        )
        .is_err());
        assert!(inventory("ini", &format!("[all:vars]\nansible_port={port}\n")).is_err());
    }
}

fn parent_store(behaviour: HashBehaviour) -> VarStore {
    let mut store = VarStore::with_hash_behaviour(behaviour);
    store.set(
        "config",
        yaml("nested: {keep: 1, shared: old}\nlist: [1]\n"),
        VarPrecedence::RoleDefaults,
    );
    store.set(
        "config",
        yaml("nested: {added: 2, shared: new}\nlist: [2]\n"),
        VarPrecedence::PlayVars,
    );
    store
}

#[test]
fn scope_reads_merged_parent_with_and_without_parent_cache() {
    for cached in [false, true] {
        let mut parent = parent_store(HashBehaviour::Merge);
        if cached {
            parent.all();
        }
        let scope = parent.scope();
        let expected = yaml("nested: {keep: 1, added: 2, shared: new}\nlist: [2]\n");
        assert_eq!(scope.get("config"), Some(&expected));
        assert_eq!(scope.all().get("config"), Some(&expected));
        assert_eq!(scope.get("missing"), None);
    }
}

#[test]
fn scope_local_overrides_replace_without_mutating_parent() {
    let mut parent = parent_store(HashBehaviour::Merge);
    let expected_parent = parent.all().clone();
    {
        let mut scope = parent.scope();
        let replacement = yaml("local: true");
        scope.set("config", replacement.clone());
        assert_eq!(scope.get("config"), Some(&replacement));
        assert_eq!(scope.all().get("config"), Some(&replacement));
    }
    assert_eq!(parent.all(), &expected_parent);
}

#[test]
fn scope_replace_policy_still_uses_highest_layer() {
    let mut parent = parent_store(HashBehaviour::Replace);
    let expected = parent.all().clone();
    let scope = parent.scope();
    assert_eq!(scope.all(), expected);
    assert_eq!(scope.get("config"), expected.get("config"));
}
