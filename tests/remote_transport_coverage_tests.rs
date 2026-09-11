//! Every registered module has a deliberate remote-transport classification.
//!
//! The executor refuses a module it does not recognise as remote-capable,
//! which is the safe default but also a silent one: a module built entirely
//! against the connection stayed unusable on remote hosts simply because
//! nobody added it to a list. This test makes the omission loud — a new module
//! must be classified as verified, connection-only, control-node-only, or
//! connectionless before it ships.

use rustible::executor::task::{RemoteTransport, Task};
use rustible::modules::ModuleRegistry;

/// Modules the executor handles before it ever reaches a module registry.
const CONNECTIONLESS: &[&str] = &[
    "assert",
    "debug",
    "fail",
    "include_vars",
    "meta",
    "pause",
    "set_fact",
];

#[test]
fn every_registered_module_is_classified() {
    let registry = ModuleRegistry::with_builtins();
    let unclassified: Vec<&str> = registry
        .names()
        .into_iter()
        .filter(|name| !CONNECTIONLESS.contains(name))
        .filter(|name| Task::remote_transport(name).is_none())
        .collect();

    assert!(
        unclassified.is_empty(),
        "these modules would be silently refused on remote hosts; add each to the verified, \
         connection-only or control-node-only list in src/executor/task.rs: {:?}",
        unclassified
    );
}

#[test]
fn the_classification_lists_do_not_overlap() {
    let registry = ModuleRegistry::with_builtins();
    for name in registry.names() {
        if CONNECTIONLESS.contains(&name) {
            assert!(
                Task::remote_transport(name).is_none(),
                "{} is handled without a connection and must not also be classified",
                name
            );
        }
    }
}

#[test]
fn classification_names_only_registered_modules() {
    let registry = ModuleRegistry::with_builtins();
    let registered: Vec<&str> = registry.names();

    // `setup` is the Ansible alias the executor resolves to `gather_facts`
    // before dispatch, so it is classified without being registered.
    let known_aliases = ["setup"];

    for name in Task::classified_modules() {
        assert!(
            registered.contains(&name) || known_aliases.contains(&name),
            "{} is classified but no module registers under that name; a stale entry silently \
             grants remote execution to nothing",
            name
        );
    }
}

#[test]
fn feature_gated_modules_are_classified_when_their_feature_is_on() {
    // The gated list is allowed to name modules this build does not register —
    // that is the point of it. What must hold is the converse: anything the
    // build *does* register from that list is classified, which the
    // every_registered_module_is_classified test above covers, and that the
    // gated list never duplicates an always-registered classification.
    let always: Vec<&str> = Task::classified_modules().collect();
    for name in Task::feature_gated_modules() {
        assert!(
            !always.contains(&name),
            "{} is classified twice: once as always-registered and once as feature-gated",
            name
        );
    }
}

#[test]
fn a_control_node_module_is_refused_with_a_useful_message() {
    // `synchronize` drives rsync from the control node: it has no connection
    // path, and the refusal should say so rather than leave the reader
    // wondering whether the transport is merely untested.
    assert_eq!(
        Task::remote_transport("synchronize"),
        Some(RemoteTransport::ControlNodeOnly)
    );
    assert_eq!(
        Task::remote_transport("copy"),
        Some(RemoteTransport::Verified)
    );
    assert_eq!(
        Task::remote_transport("service"),
        Some(RemoteTransport::ConnectionOnly)
    );
}
