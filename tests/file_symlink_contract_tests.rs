//! Local file-module regressions confined to fresh temporary directories.
//! Child test processes isolate cwd; no real inventory or external command runs.

use rustible::modules::{file::FileModule, Module, ModuleContext, ModuleParams};
use serde_json::json;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

fn write_file(path: &Path, contents: &str, permissions: u32) {
    fs::write(path, contents).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(permissions)).unwrap();
}

fn run_fixture(cwd: &Path, params: serde_json::Value, outcome: &str, check: bool) {
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "file_contract_subprocess", "--nocapture"])
        .current_dir(cwd)
        .env("RUSTIBLE_FILE_FIXTURE_PARAMS", params.to_string())
        .env("RUSTIBLE_FILE_FIXTURE_OUTCOME", outcome)
        .env("RUSTIBLE_FILE_FIXTURE_CHECK", check.to_string())
        // A baseline SELinux probe must not execute any installed system tool.
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
}

#[test]
#[ignore = "helper invoked only by the isolated temporary-directory tests"]
fn file_contract_subprocess() {
    let params: ModuleParams = serde_json::from_str(
        &std::env::var("RUSTIBLE_FILE_FIXTURE_PARAMS").expect("fixture parameters"),
    )
    .unwrap();
    let check = std::env::var("RUSTIBLE_FILE_FIXTURE_CHECK").unwrap() == "true";
    let result = FileModule.execute(&params, &ModuleContext::default().with_check_mode(check));
    match std::env::var("RUSTIBLE_FILE_FIXTURE_OUTCOME")
        .unwrap()
        .as_str()
    {
        "changed" => assert!(result.unwrap().changed),
        "unchanged" => assert!(!result.unwrap().changed),
        "unsupported" => assert!(matches!(
            result,
            Err(rustible::modules::ModuleError::Unsupported(_))
        )),
        "error" => assert!(result.is_err()),
        _ => panic!("unknown fixture outcome"),
    }
}

#[test]
fn relative_file_links_resolve_from_their_parent_and_are_idempotent() {
    for relative_path in [false, true] {
        let temp = TempDir::new().unwrap();
        let links = temp.path().join("links");
        fs::create_dir(&links).unwrap();
        let target = links.join("target");
        let unrelated = temp.path().join("target");
        write_file(&target, "intended", 0o644);
        write_file(&unrelated, "cwd sentinel", 0o640);
        symlink("target", links.join("link")).unwrap();
        let path = if relative_path {
            "links/link".into()
        } else {
            links.join("link")
        };
        let params = json!({"path": path, "state": "file", "follow": true, "mode": 0o600});
        run_fixture(temp.path(), params.clone(), "changed", false);
        assert_eq!(mode(&target), 0o600);
        assert_eq!(mode(&unrelated), 0o640);
        assert_eq!(fs::read_to_string(&target).unwrap(), "intended");
        assert_eq!(fs::read_to_string(&unrelated).unwrap(), "cwd sentinel");
        run_fixture(temp.path(), params, "unchanged", false);
    }
}

#[test]
fn following_links_handles_absolute_chained_and_dangling_targets() {
    for kind in ["absolute", "chain", "dangling"] {
        let temp = TempDir::new().unwrap();
        let links = temp.path().join("links");
        fs::create_dir(&links).unwrap();
        let target = links.join("nested/target");
        if kind != "dangling" {
            fs::create_dir(links.join("nested")).unwrap();
            write_file(&target, "intended", 0o644);
        }
        match kind {
            "absolute" => symlink(&target, links.join("link")).unwrap(),
            "chain" => {
                symlink("nested/target", links.join("second")).unwrap();
                symlink("second", links.join("link")).unwrap();
            }
            _ => symlink("nested/target", links.join("link")).unwrap(),
        }
        let params =
            json!({"path": links.join("link"), "state": "file", "follow": true, "mode": 0o600});
        run_fixture(temp.path(), params, "changed", false);
        assert_eq!(mode(&target), 0o600, "{kind}");
        assert!(!temp.path().join("nested").exists(), "{kind} touched cwd");
        assert!(!temp.path().join("second").exists(), "{kind} touched cwd");
    }
}

#[test]
fn no_follow_file_keeps_existing_and_dangling_targets_unchanged() {
    for exists in [false, true] {
        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target");
        if exists {
            write_file(&target, "sentinel", 0o640);
        }
        let link = temp.path().join("link");
        symlink("target", &link).unwrap();
        let params = json!({"path": link, "state": "file", "follow": false, "mode": 0o600});
        run_fixture(temp.path(), params.clone(), "unchanged", true);
        run_fixture(temp.path(), params, "unchanged", false);
        assert_eq!(target.exists(), exists);
        if exists {
            assert_eq!(mode(&target), 0o640);
            assert_eq!(fs::read_to_string(&target).unwrap(), "sentinel");
        }
    }
}

fn unsupported_request_preserves_attributes(option: &str) {
    for check in [false, true] {
        let temp = TempDir::new().unwrap();
        let tree = temp.path().join("tree");
        fs::create_dir(&tree).unwrap();
        fs::set_permissions(&tree, fs::Permissions::from_mode(0o700)).unwrap();
        let target = temp.path().join("target");
        write_file(&target, "sentinel", 0o640);
        filetime::set_file_mtime(&target, filetime::FileTime::from_unix_time(123, 0)).unwrap();
        let link = tree.join("link");
        symlink("../target", &link).unwrap();
        let mut params = json!({"path": link, "state": "file", "follow": false, "mode": 0o600});
        match option {
            "time" => params["modification_time"] = json!("456"),
            "touch" => params["state"] = json!("touch"),
            "selinux" => params["setype"] = json!("synthetic_type"),
            _ => {
                params["path"] = json!(tree);
                params["state"] = json!("directory");
                params["mode"] = json!(0o755);
                params["setype"] = json!("synthetic_type");
                params["recurse"] = json!(true);
            }
        }
        run_fixture(temp.path(), params, "unsupported", check);
        assert_eq!(mode(&target), 0o640);
        assert_eq!(mode(&tree), 0o700);
        assert_eq!(fs::metadata(&target).unwrap().mtime(), 123);
        assert_eq!(fs::read_to_string(&target).unwrap(), "sentinel");
    }
}

#[test]
fn no_follow_timestamp_request_fails_before_mutation() {
    unsupported_request_preserves_attributes("time");
}

#[test]
fn no_follow_touch_request_fails_before_mutation() {
    unsupported_request_preserves_attributes("touch");
}

#[test]
fn no_follow_selinux_request_fails_before_mutation() {
    unsupported_request_preserves_attributes("selinux");
}

#[test]
fn no_follow_recursive_selinux_request_fails_before_mutation() {
    unsupported_request_preserves_attributes("recursive_selinux");
}

/// A group the current process may assign that differs from `current`.
#[cfg(not(target_vendor = "apple"))]
fn different_permitted_group(current: u32) -> Option<u32> {
    nix::unistd::getgroups()
        .unwrap()
        .into_iter()
        .map(|gid| gid.as_raw())
        .find(|gid| *gid != current)
}

/// nix does not expose `getgroups` on Apple platforms; ask `id -G` instead.
#[cfg(target_vendor = "apple")]
fn different_permitted_group(current: u32) -> Option<u32> {
    let output = Command::new("id").arg("-G").output().unwrap();
    assert!(output.status.success(), "id -G failed");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .filter_map(|gid| gid.parse::<u32>().ok())
        .find(|gid| *gid != current)
}

#[test]
fn recursive_no_follow_ownership_updates_link_without_target_changes() {
    let temp = TempDir::new().unwrap();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let target = temp.path().join("target");
    write_file(&target, "sentinel", 0o600);
    let link = tree.join("link");
    symlink("../target", &link).unwrap();
    let initial = fs::symlink_metadata(&link).unwrap().gid();
    let Some(group) = different_permitted_group(initial) else {
        eprintln!("SKIP ownership change: no different supplementary group is available");
        return;
    };
    let params = json!({"path": tree, "state": "directory", "recurse": true, "follow": false, "group": group});
    run_fixture(temp.path(), params.clone(), "changed", false);
    assert_eq!(fs::symlink_metadata(&link).unwrap().gid(), group);
    assert_eq!(fs::metadata(&target).unwrap().gid(), initial);
    assert_eq!(fs::read_to_string(&target).unwrap(), "sentinel");
    run_fixture(temp.path(), params, "unchanged", false);
}

#[test]
fn recursive_follow_ownership_uses_target_metadata_for_idempotency() {
    let temp = TempDir::new().unwrap();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let target = temp.path().join("target");
    write_file(&target, "sentinel", 0o600);
    let link = tree.join("link");
    symlink("../target", &link).unwrap();
    let initial = fs::metadata(&target).unwrap().gid();
    let Some(group) = different_permitted_group(initial) else {
        eprintln!("SKIP ownership change: no different supplementary group is available");
        return;
    };
    let params = json!({"path": tree, "state": "directory", "recurse": true, "follow": true, "group": group});
    run_fixture(temp.path(), params.clone(), "changed", false);
    assert_eq!(fs::metadata(&target).unwrap().gid(), group);
    assert_eq!(fs::symlink_metadata(&link).unwrap().gid(), initial);
    run_fixture(temp.path(), params, "unchanged", false);
}

#[test]
fn followed_symlink_cycles_fail_without_creating_files() {
    let temp = TempDir::new().unwrap();
    symlink("second", temp.path().join("first")).unwrap();
    symlink("first", temp.path().join("second")).unwrap();
    run_fixture(
        temp.path(),
        json!({"path": temp.path().join("first"), "state": "file", "follow": true}),
        "error",
        false,
    );
    assert!(temp.path().join("first").is_symlink());
    assert!(temp.path().join("second").is_symlink());
}

#[test]
fn check_mode_leaves_relative_link_targets_unchanged() {
    let temp = TempDir::new().unwrap();
    let tree = temp.path().join("tree");
    fs::create_dir(&tree).unwrap();
    let target = tree.join("target");
    write_file(&target, "sentinel", 0o640);
    symlink("target", tree.join("link")).unwrap();
    run_fixture(
        temp.path(),
        json!({"path": tree.join("link"), "state": "file", "follow": true, "mode": 0o600}),
        "changed",
        true,
    );
    assert_eq!(mode(&target), 0o640);
    assert_eq!(fs::read_to_string(&target).unwrap(), "sentinel");
    assert!(!temp.path().join("target").exists());
}

#[test]
fn recursive_modes_follow_links_only_when_requested_including_root_links() {
    for root_link in [false, true] {
        for follow in [false, true] {
            let temp = TempDir::new().unwrap();
            let tree = temp.path().join("tree");
            let target = temp.path().join("target");
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
            let inside = target.join("file");
            write_file(&inside, "sentinel", 0o600);
            if root_link {
                symlink("target", &tree).unwrap();
            } else {
                fs::create_dir(&tree).unwrap();
                fs::set_permissions(&tree, fs::Permissions::from_mode(0o755)).unwrap();
                symlink("../target", tree.join("link")).unwrap();
            }
            run_fixture(
                temp.path(),
                json!({"path": tree, "state": "directory", "follow": follow, "recurse": true, "mode": 0o755}),
                if follow { "changed" } else { "unchanged" },
                false,
            );
            assert_eq!(mode(&target), if follow { 0o755 } else { 0o700 });
            assert_eq!(mode(&inside), if follow { 0o755 } else { 0o600 });
            assert_eq!(fs::read_to_string(&inside).unwrap(), "sentinel");
        }
    }
}

#[test]
fn link_chain_limit_accepts_forty_and_rejects_forty_one() {
    for count in [40, 41] {
        let temp = TempDir::new().unwrap();
        write_file(&temp.path().join("target"), "sentinel", 0o600);
        for index in 0..count {
            let next = if index + 1 == count {
                "target".into()
            } else {
                format!("link-{}", index + 1)
            };
            symlink(next, temp.path().join(format!("link-{index}"))).unwrap();
        }
        run_fixture(
            temp.path(),
            json!({"path": temp.path().join("link-0"), "state": "file", "follow": true}),
            if count == 40 { "unchanged" } else { "error" },
            false,
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("target")).unwrap(),
            "sentinel"
        );
    }
}
