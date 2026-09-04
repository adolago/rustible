//! The policy CLI reads this temporary YAML; it never executes the task.
use std::time::Duration;

#[test]
fn cli_does_not_claim_success_when_builtin_checks_are_unavailable() {
    let directory = tempfile::tempdir().unwrap();
    let playbook = directory.path().join("fixture.yml");
    std::fs::write(&playbook, "- name: Fixture\n  hosts: localhost\n  tags: [fixture]\n  tasks:\n    - name: Fixture\n      debug:\n        msg: fixture\n").unwrap();
    let output = assert_cmd::Command::new(env!("CARGO_BIN_EXE_rustible"))
        .current_dir(directory.path())
        .args(["--no-color", "policy", "check"])
        .arg(&playbook)
        .timeout(Duration::from_secs(10))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("unsupported"));
    assert!(!text.contains("All policy checks passed"));
}
