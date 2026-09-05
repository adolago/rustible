//! Init only writes starter files beneath a fresh temporary directory.
//! These tests never execute the generated package/service/user tasks.

use rustible::executor::playbook::Playbook;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn privileged_starters_use_the_yaml_become_contract() {
    for template in ["webserver", "docker"] {
        let directory = tempdir().unwrap();
        assert_cmd::Command::new(env!("CARGO_BIN_EXE_rustible"))
            .current_dir(directory.path())
            .args(["init", ".", "--template", template])
            .timeout(Duration::from_secs(10))
            .assert()
            .success();
        let path = directory.path().join("playbooks/site.yml");
        let content = std::fs::read_to_string(&path).unwrap();
        let value: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
        let play = value.as_sequence().unwrap()[0].as_mapping().unwrap();
        assert_eq!(
            play.get(serde_yaml::Value::String("become".into()))
                .and_then(serde_yaml::Value::as_bool),
            Some(true)
        );
        assert!(!play.contains_key(serde_yaml::Value::String("r#become".into())));
        assert!(Playbook::load(path).unwrap().plays[0].r#become);
    }
}
