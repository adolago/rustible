//! Policy evaluation fixtures are JSON data only; no task is executed.

use rustible::policy::pack::{PackLoader, PackRegistry, PackRule, RuleCheck};
use rustible::policy::RuleSeverity;
use serde_json::json;

fn manifest(rules: &[&str]) -> String {
    format!(
        "name: fixture\nversion: '1.0.0'\ndescription: Synthetic policy fixture\ncategory: Security\nrules:\n{}parameters: []\n",
        rules.iter().map(|name| format!("  - {name}\n")).collect::<String>()
    )
}

fn harmless_play() -> serde_json::Value {
    json!([{"hosts": "localhost", "tasks": [{"name": "fixture", "debug": {"msg": "fixture"}}]}])
}

#[test]
fn unknown_rule_is_reported_as_unsupported_and_failed() {
    let mut registry = PackRegistry::new();
    registry.load(&manifest(&["unknown-fixture-rule"])).unwrap();
    let result = registry.evaluate_all(&harmless_play()).remove(0);
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 1);
    assert_eq!(result.warnings, 0);
    assert!(result
        .details
        .iter()
        .any(|detail| detail.contains("unsupported")));
}

#[test]
fn every_unimplemented_named_rule_fails_without_claiming_enforcement() {
    for name in [
        "require-become-explicit",
        "max-forks",
        "require-limit",
        "deny-localhost-in-prod",
    ] {
        let mut registry = PackRegistry::new();
        registry.load(&manifest(&[name])).unwrap();
        let result = registry.evaluate_all(&harmless_play()).remove(0);
        assert_eq!(result.passed, 0, "{name}");
        assert_eq!(result.failed, 1, "{name}");
        assert!(
            result
                .details
                .iter()
                .any(|detail| detail.contains("unsupported")),
            "{name}"
        );
    }
}

#[test]
fn direct_custom_check_never_returns_an_empty_success() {
    let rule = PackRule {
        name: "fixture".into(),
        description: "fixture".into(),
        severity: RuleSeverity::Info,
        check: RuleCheck::Custom("fixture".into()),
    };
    assert!(!rule.evaluate(&harmless_play()).is_empty());
}

#[test]
fn discovered_operations_pack_has_three_unavailable_checks() {
    let mut registry = PackRegistry::new();
    registry.discover();
    let results = registry.evaluate_all(&harmless_play());
    let result = results
        .iter()
        .find(|result| result.pack_name == "operations-baseline")
        .unwrap();
    assert_eq!(result.passed, 0);
    assert_eq!(result.failed, 3);
    assert!(result
        .details
        .iter()
        .all(|detail| detail.contains("unsupported")));
}

#[test]
fn implemented_rule_still_accepts_its_positive_control() {
    let mut registry = PackRegistry::new();
    registry.load(&manifest(&["no-shell"])).unwrap();
    let result = registry.evaluate_all(&harmless_play()).remove(0);
    assert_eq!(result.passed, 1);
    assert_eq!(result.failed, 0);
    assert!(result.details.is_empty());
}

#[test]
fn implemented_rule_still_reports_its_negative_control() {
    let pack = PackLoader::load_from_manifest(&manifest(&["no-shell"])).unwrap();
    let input = json!([{"hosts": "localhost", "tasks": [{"name": "fixture", "shell": "literal fixture; never executed"}]}]);
    assert_eq!(pack.rules[0].evaluate(&input).len(), 1);
}
