//! File attribute contracts using temporary files and an in-memory transport only.
#![cfg(unix)]

use async_trait::async_trait;
use rustible::connection::{
    CommandResult, Connection, ConnectionError, ConnectionResult, ExecuteOptions, FileStat,
    TransferOptions,
};
use rustible::modules::blockinfile::BlockinfileModule;
use rustible::modules::lineinfile::LineinfileModule;
use rustible::modules::stat::StatModule;
use rustible::modules::{Module, ModuleContext, ModuleParams};
use serde_json::json;
use std::fs;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

fn params(path: &Path) -> ModuleParams {
    [("path".to_string(), json!(path.to_str().unwrap()))]
        .into_iter()
        .collect()
}

fn permission_only(module: &dyn Module, content: &str, field: &str, value: &str) {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("document");
    fs::write(&path, content).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let before = fs::metadata(&path).unwrap();
    let mut request = params(&path);
    request.insert(field.to_string(), json!(value));
    request.insert("mode".to_string(), json!("0600"));
    request.insert("backup".to_string(), json!(true));

    let check = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);
    let preview = module.execute(&request, &check).unwrap();
    assert!(preview.changed);
    assert!(preview.diff.is_none());
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o644);
    assert_eq!(fs::read(&path).unwrap(), content.as_bytes());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);

    let result = module.execute(&request, &ModuleContext::default()).unwrap();
    assert!(result.changed);
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
    assert_eq!(fs::read(&path).unwrap(), content.as_bytes());
    let after = fs::metadata(&path).unwrap();
    assert_eq!(
        (after.ino(), after.mtime(), after.mtime_nsec()),
        (before.ino(), before.mtime(), before.mtime_nsec())
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
    assert!(
        !module
            .execute(&request, &ModuleContext::default())
            .unwrap()
            .changed
    );
    assert!(!module.execute(&request, &check).unwrap().changed);
}

#[test]
fn line_mode_only_preserves_bytes_and_honors_check_mode() {
    permission_only(&LineinfileModule, "keep\r\nselected", "line", "selected");
}

#[test]
fn block_mode_only_preserves_bytes_and_honors_check_mode() {
    permission_only(
        &BlockinfileModule,
        "# BEGIN ANSIBLE MANAGED BLOCK\r\nselected\r\n# END ANSIBLE MANAGED BLOCK",
        "block",
        "selected",
    );
}

fn absent_mode(module: &dyn Module, field: &str) {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("document");
    fs::write(&path, b"unrelated").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let mut request = params(&path);
    request.insert("state".to_string(), json!("absent"));
    request.insert(field.to_string(), json!("missing"));
    request.insert("mode".to_string(), json!("0600"));
    assert!(
        module
            .execute(&request, &ModuleContext::default())
            .unwrap()
            .changed
    );
    assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
    assert_eq!(fs::read(&path).unwrap(), b"unrelated");
}

#[test]
fn absent_line_still_applies_requested_mode() {
    absent_mode(&LineinfileModule, "line");
}

#[test]
fn absent_block_still_applies_requested_mode() {
    absent_mode(&BlockinfileModule, "block");
}

#[test]
fn mode_unspecified_keeps_existing_local_permissions() {
    for (module, field, content) in [
        (&LineinfileModule as &dyn Module, "line", "selected"),
        (
            &BlockinfileModule as &dyn Module,
            "block",
            "# BEGIN ANSIBLE MANAGED BLOCK\nselected\n# END ANSIBLE MANAGED BLOCK",
        ),
    ] {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("document");
        fs::write(&path, content).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let mut request = params(&path);
        request.insert(field.to_string(), json!("selected"));
        assert!(
            !module
                .execute(&request, &ModuleContext::default())
                .unwrap()
                .changed
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o640);
        assert_eq!(fs::read(&path).unwrap(), content.as_bytes());
    }
}

#[test]
fn content_and_mode_updates_remain_supported_locally() {
    for (module, field) in [
        (&LineinfileModule as &dyn Module, "line"),
        (&BlockinfileModule as &dyn Module, "block"),
    ] {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("document");
        fs::write(&path, "original\n").unwrap();
        let mut request = params(&path);
        request.insert(field.to_string(), json!("selected"));
        request.insert("mode".to_string(), json!("0600"));
        assert!(
            module
                .execute(&request, &ModuleContext::default())
                .unwrap()
                .changed
        );
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o7777, 0o600);
        assert!(fs::read_to_string(&path).unwrap().contains("selected"));
    }
}

fn stat(path: &Path, follow: bool) -> serde_json::Value {
    let mut request = params(path);
    request.insert("follow".to_string(), json!(follow));
    StatModule
        .execute(&request, &ModuleContext::default())
        .unwrap()
        .data["stat"]
        .clone()
}

#[test]
fn stat_mode_omits_type_bits_and_keeps_special_permission_bits() {
    let temp = TempDir::new().unwrap();
    let file = temp.path().join("document");
    fs::write(&file, "content").unwrap();
    fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(stat(&file, true)["mode"], "0644");
    let directory = temp.path().join("directory");
    fs::create_dir(&directory).unwrap();
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o1700)).unwrap();
    assert_eq!(stat(&directory, true)["mode"], "1700");
}

#[test]
fn stat_unfollowed_relative_link_reports_raw_target() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("target"), "content").unwrap();
    let link = temp.path().join("link");
    symlink("target", &link).unwrap();
    let value = stat(&link, false);
    assert_eq!(value["islnk"], true);
    assert_eq!(value["isreg"], false);
    assert_eq!(value["lnk_source"], "target");
    assert_eq!(
        value["mode"],
        format!(
            "{:04o}",
            fs::symlink_metadata(&link).unwrap().mode() & 0o7777
        )
    );
}

#[test]
fn stat_dangling_link_exists_only_when_not_followed() {
    let temp = TempDir::new().unwrap();
    let link = temp.path().join("link");
    symlink("missing-target", &link).unwrap();
    assert_eq!(stat(&link, false)["lnk_source"], "missing-target");
    assert_eq!(stat(&link, false)["exists"], true);
    assert_eq!(stat(&link, true)["exists"], false);
}

#[test]
fn stat_followed_link_describes_target_without_link_source() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("target");
    fs::write(&target, "content").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();
    let link = temp.path().join("link");
    symlink("target", &link).unwrap();
    let value = stat(&link, true);
    assert_eq!(value["isreg"], true);
    assert_eq!(value["islnk"], false);
    assert_eq!(value["size"], 7);
    assert!(value.get("lnk_source").is_none());
    assert_eq!(value["mode"], "0640");
}

#[derive(Default)]
struct MemoryConnection {
    uploads: Mutex<Vec<Vec<u8>>>,
}

#[async_trait]
impl Connection for MemoryConnection {
    fn identifier(&self) -> &str {
        "in-memory-only"
    }
    async fn is_alive(&self) -> bool {
        true
    }
    async fn execute(&self, _: &str, _: Option<ExecuteOptions>) -> ConnectionResult<CommandResult> {
        panic!("No command execution is permitted in this fixture")
    }
    async fn upload(&self, _: &Path, _: &Path, _: Option<TransferOptions>) -> ConnectionResult<()> {
        panic!("File uploads are not used")
    }
    async fn upload_content(
        &self,
        bytes: &[u8],
        _: &Path,
        _: Option<TransferOptions>,
    ) -> ConnectionResult<()> {
        self.uploads.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
    async fn download(&self, _: &Path, _: &Path) -> ConnectionResult<()> {
        panic!("No disk downloads")
    }
    async fn download_content(&self, _: &Path) -> ConnectionResult<Vec<u8>> {
        Ok(b"selected".to_vec())
    }
    async fn path_exists(&self, _: &Path) -> ConnectionResult<bool> {
        Ok(true)
    }
    async fn is_directory(&self, _: &Path) -> ConnectionResult<bool> {
        Ok(false)
    }
    async fn stat(&self, _: &Path) -> ConnectionResult<FileStat> {
        Err(ConnectionError::UnsupportedOperation(
            "No portable mode operation".to_string(),
        ))
    }
    async fn close(&self) -> ConnectionResult<()> {
        Ok(())
    }
}

#[test]
fn remote_mode_only_is_rejected_without_transfer_including_check_mode() {
    for check_mode in [false, true] {
        let connection = Arc::new(MemoryConnection::default());
        let mut request = params(Path::new("synthetic-document"));
        request.insert("line".to_string(), json!("selected"));
        request.insert("mode".to_string(), json!("0600"));
        request.insert("backup".to_string(), json!(true));
        let context = ModuleContext::default()
            .with_connection(connection.clone())
            .with_check_mode(check_mode);
        let error = LineinfileModule.execute(&request, &context).unwrap_err();
        assert!(error.to_string().contains("mode"));
        assert!(error.to_string().contains("unsupported"));
        assert!(connection.uploads.lock().unwrap().is_empty());
    }
}

#[test]
fn remote_unchanged_content_without_mode_remains_a_noop() {
    let connection = Arc::new(MemoryConnection::default());
    let mut request = params(Path::new("synthetic-document"));
    request.insert("line".to_string(), json!("selected"));
    let context = ModuleContext::default().with_connection(connection.clone());
    assert!(
        !LineinfileModule
            .execute(&request, &context)
            .unwrap()
            .changed
    );
    assert!(connection.uploads.lock().unwrap().is_empty());
}

#[test]
fn remote_content_change_without_mode_keeps_existing_transfer_path() {
    let connection = Arc::new(MemoryConnection::default());
    let mut request = params(Path::new("synthetic-document"));
    request.insert("line".to_string(), json!("additional"));
    let context = ModuleContext::default().with_connection(connection.clone());
    assert!(
        LineinfileModule
            .execute(&request, &context)
            .unwrap()
            .changed
    );
    assert_eq!(
        *connection.uploads.lock().unwrap(),
        [b"selected\nadditional\n".to_vec()]
    );
}
