//! Native local lookups used in place of shelling out.
//!
//! When a task targets the control node, user, group and package questions are
//! answered from the system databases directly. These tests check that the
//! native answer matches what the shell command it replaces would report on
//! this machine.

use std::process::Command;

use rustible::connection::local::LocalConnection;
use rustible::connection::Connection;
use rustible::native::{apt, users};

/// Output of a command, or `None` when it is unavailable or fails.
fn shell_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[test]
fn local_connection_reports_itself_as_local() {
    let local = LocalConnection::new();
    assert!(local.is_local(), "the local transport runs on this machine");
}

#[test]
fn native_user_lookup_matches_getent() {
    let Some(passwd_line) = shell_output("getent", &["passwd", "root"]) else {
        eprintln!("skipping: getent is unavailable");
        return;
    };
    let fields: Vec<&str> = passwd_line.split(':').collect();
    assert!(
        fields.len() >= 7,
        "unexpected passwd format: {}",
        passwd_line
    );

    let user = users::get_user_by_name("root")
        .expect("reading /etc/passwd should succeed")
        .expect("root should exist");

    assert_eq!(user.name, fields[0]);
    assert_eq!(user.uid.to_string(), fields[2]);
    assert_eq!(user.gid.to_string(), fields[3]);
    assert_eq!(user.home, fields[5]);
    assert_eq!(user.shell, fields[6]);
}

#[test]
fn native_user_lookup_reports_missing_users() {
    let missing = users::get_user_by_name("rustible-no-such-user-9c1f")
        .expect("reading /etc/passwd should succeed");
    assert!(missing.is_none());
}

#[test]
fn native_group_lookup_matches_getent() {
    let Some(group_line) = shell_output("getent", &["group", "root"]) else {
        eprintln!("skipping: getent is unavailable");
        return;
    };
    let fields: Vec<&str> = group_line.split(':').collect();
    assert!(fields.len() >= 3, "unexpected group format: {}", group_line);

    let group = users::get_group_by_name("root")
        .expect("reading /etc/group should succeed")
        .expect("the root group should exist");

    assert_eq!(group.name, fields[0]);
    assert_eq!(group.gid.to_string(), fields[2]);
}

#[test]
fn native_dpkg_version_matches_dpkg_query() {
    if !std::path::Path::new("/var/lib/dpkg/status").exists() {
        eprintln!("skipping: not a dpkg system");
        return;
    }
    // coreutils is present on every Debian-family system that has dpkg.
    let Some(expected) = shell_output("dpkg-query", &["-W", "-f=${Version}", "coreutils"]) else {
        eprintln!("skipping: dpkg-query is unavailable");
        return;
    };

    let mut native = apt::AptNative::new().expect("the dpkg status file should be readable");
    let version = native
        .get_version("coreutils")
        .expect("reading the status file should succeed");

    assert_eq!(version.as_deref(), Some(expected.as_str()));
    assert!(native
        .is_installed("coreutils")
        .expect("reading the status file should succeed"));
}

#[test]
fn native_dpkg_reports_unknown_packages() {
    if !std::path::Path::new("/var/lib/dpkg/status").exists() {
        eprintln!("skipping: not a dpkg system");
        return;
    }
    let mut native = apt::AptNative::new().expect("the dpkg status file should be readable");
    assert!(!native
        .is_installed("rustible-no-such-package-9c1f")
        .expect("reading the status file should succeed"));
}
