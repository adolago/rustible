#![cfg(unix)]
//! Fake CLI only: recorded remote strings are never evaluated or forwarded.

use rustible::connection::docker::DockerConnection;
use rustible::connection::podman::PodmanConnection;
use rustible::connection::{Connection, TransferOptions};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

struct Fixture {
    dir: tempfile::TempDir,
    log: PathBuf,
    connection: Box<dyn Connection>,
}

impl Fixture {
    fn new(docker: bool, failure: &str, probe: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("calls");
        let executable = dir.path().join("fake-cli");
        let script = format!(
            r#"#!/bin/sh
log={log}
printf '%s\000' "$@" >> "$log"
printf '\000' >> "$log"
case "$1" in
  inspect) printf true; exit 0 ;;
  cp) stage=cp ;;
  exec)
    for argument do remote=$argument; done
    case "$remote" in
      mkdir\ *) stage=mkdir ;;
      chmod\ *) stage=chmod ;;
      chown\ *) stage=chown ;;
      cat\ *) stage=read ;;
      stat\ *) stage=stat ;;
      test\ *) stage=probe ;;
      *) exit 91 ;;
    esac ;;
  *) exit 92 ;;
esac
if [ "$stage" = {failure} ]; then printf 'synthetic failure' >&2; exit 23; fi
case "$stage" in
  read) printf 'synthetic content' ;;
  stat) printf '7|644|1000|1000|1|2|regular file' ;;
  probe) printf '%s' {probe} ;;
esac
exit 0
"#,
            log = rustible::utils::shell_escape(log.to_str().unwrap()),
            failure = rustible::utils::shell_escape(failure),
            probe = rustible::utils::shell_escape(probe),
        );
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        let executable = executable.to_str().unwrap();
        let connection: Box<dyn Connection> = if docker {
            Box::new(DockerConnection::with_docker_path("synthetic", executable))
        } else {
            Box::new(PodmanConnection::with_podman_path("synthetic", executable))
        };
        Self {
            dir,
            log,
            connection,
        }
    }

    fn calls(&self) -> Vec<Vec<String>> {
        let bytes = std::fs::read(&self.log).unwrap_or_default();
        let mut calls = Vec::new();
        let mut call = Vec::new();
        for field in bytes.split(|byte| *byte == 0) {
            if field.is_empty() {
                if !call.is_empty() {
                    calls.push(std::mem::take(&mut call));
                }
            } else {
                call.push(String::from_utf8(field.to_vec()).unwrap());
            }
        }
        assert!(call.is_empty());
        calls
    }

    fn commands(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .filter(|call| call[0] == "exec")
            .map(|call| shell_words::split(call.last().unwrap()).unwrap())
            .collect()
    }

    fn stages(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call[0].as_str() {
                "cp" => Some("cp".into()),
                "exec" => Some(shell_words::split(call.last().unwrap()).unwrap()[0].clone()),
                _ => None,
            })
            .collect()
    }

    fn source(&self) -> PathBuf {
        let path = self.dir.path().join("local file's; marker");
        std::fs::write(&path, b"synthetic").unwrap();
        path
    }
}

fn options() -> TransferOptions {
    TransferOptions::new()
        .with_create_dirs()
        .with_mode(0o640)
        .with_owner("user's value; marker")
        .with_group("group value")
}

async fn upload_literals(docker: bool) {
    let f = Fixture::new(docker, "none", "yes");
    let source = f.source();
    let remote = Path::new("/literal dir/file's; marker\nnext");
    f.connection
        .upload(&source, remote, Some(options()))
        .await
        .unwrap();
    assert_eq!(f.stages(), ["mkdir", "cp", "chmod", "chown"]);
    let commands = f.commands();
    assert_eq!(commands[0], ["mkdir", "-p", "--", "/literal dir"]);
    assert_eq!(
        commands[1],
        ["chmod", "640", "--", remote.to_str().unwrap()]
    );
    assert_eq!(
        commands[2],
        [
            "chown",
            "--",
            "user's value; marker:group value",
            remote.to_str().unwrap()
        ]
    );
    let cp = f.calls().into_iter().find(|call| call[0] == "cp").unwrap();
    assert_eq!(
        cp,
        [
            "cp",
            "--",
            source.to_str().unwrap(),
            &format!("synthetic:{}", remote.display())
        ]
    );
}

async fn failure_stops(docker: bool, stage: &str, expected: &[&str]) {
    let f = Fixture::new(docker, stage, "yes");
    let result = f
        .connection
        .upload(&f.source(), Path::new("/dir/file"), Some(options()))
        .await;
    assert!(result.is_err(), "{stage} failure must fail the transfer");
    assert_eq!(f.stages(), expected);
}

async fn query_literals(docker: bool) {
    let f = Fixture::new(docker, "none", "yes");
    let path = Path::new("/literal dir/file's; marker\nnext");
    assert_eq!(
        f.connection.download_content(path).await.unwrap(),
        b"synthetic content"
    );
    assert!(f.connection.path_exists(path).await.unwrap());
    assert!(f.connection.is_directory(path).await.unwrap());
    assert_eq!(f.connection.stat(path).await.unwrap().size, 7);
    let commands = f.commands();
    assert_eq!(commands[0], ["cat", "--", path.to_str().unwrap()]);
    assert_eq!(commands[1][..3], ["test", "-e", path.to_str().unwrap()]);
    assert_eq!(commands[2][..3], ["test", "-d", path.to_str().unwrap()]);
    assert_eq!(
        commands[3],
        [
            "stat",
            "-c",
            "%s|%a|%u|%g|%X|%Y|%F",
            "--",
            path.to_str().unwrap()
        ]
    );
    let negative = Fixture::new(docker, "none", "no");
    assert!(!negative.connection.path_exists(path).await.unwrap());
    assert!(!negative.connection.is_directory(path).await.unwrap());
}

async fn failed_queries(docker: bool) {
    let f = Fixture::new(docker, "probe", "yes");
    assert!(f.connection.path_exists(Path::new("/file")).await.is_err());
    assert!(f.connection.is_directory(Path::new("/file")).await.is_err());
    let f = Fixture::new(docker, "read", "yes");
    assert!(f
        .connection
        .download_content(Path::new("/file"))
        .await
        .is_err());
    let f = Fixture::new(docker, "stat", "yes");
    assert!(f.connection.stat(Path::new("/file")).await.is_err());
    let f = Fixture::new(docker, "none", "unexpected");
    assert!(f.connection.path_exists(Path::new("/file")).await.is_err());
    assert!(f.connection.is_directory(Path::new("/file")).await.is_err());
}

async fn relative_operands(docker: bool) {
    for local in ["-", "-option", "file:name"] {
        let f = Fixture::new(docker, "none", "yes");
        f.connection
            .upload(Path::new(local), Path::new("-"), Some(options()))
            .await
            .unwrap();
        let cp = f.calls().into_iter().find(|call| call[0] == "cp").unwrap();
        assert_eq!(cp, ["cp", "--", &format!("./{local}"), "synthetic:/-"]);
        let commands = f.commands();
        assert_eq!(commands.last().unwrap().last().unwrap(), "/-");
        f.connection.download_content(Path::new("-")).await.unwrap();
        assert_eq!(f.commands().last().unwrap(), &["cat", "--", "/-"]);
    }
    let f = Fixture::new(docker, "none", "yes");
    f.connection
        .upload(
            Path::new("local"),
            Path::new("dir/../file"),
            Some(options()),
        )
        .await
        .unwrap();
    let cp = f.calls().into_iter().find(|call| call[0] == "cp").unwrap();
    assert_eq!(cp.last().unwrap(), "synthetic:/dir/../file");
    assert_eq!(f.commands().last().unwrap().last().unwrap(), "/dir/../file");
}

async fn download_operands(docker: bool) {
    let f = Fixture::new(docker, "none", "yes");
    f.connection
        .download(Path::new("-"), Path::new("-"))
        .await
        .unwrap();
    assert_eq!(f.stages(), ["cp"]);
    let cp = f.calls().into_iter().find(|call| call[0] == "cp").unwrap();
    assert_eq!(cp, ["cp", "--", "synthetic:/-", "./-"]);
}

async fn invalid_paths_preflight(docker: bool) {
    for invalid in [
        PathBuf::from("bad\0path"),
        PathBuf::from(std::ffi::OsString::from_vec(vec![0xff])),
    ] {
        let f = Fixture::new(docker, "none", "yes");
        assert!(f
            .connection
            .upload(&f.source(), &invalid, Some(options()))
            .await
            .is_err());
        assert!(f
            .connection
            .upload_content(b"synthetic", &invalid, Some(options()))
            .await
            .is_err());
        assert!(f
            .connection
            .download(&invalid, &f.dir.path().join("absent/file"))
            .await
            .is_err());
        assert!(!f.dir.path().join("absent").exists());
        assert!(f.connection.download_content(&invalid).await.is_err());
        assert!(f.connection.path_exists(&invalid).await.is_err());
        assert!(f.connection.is_directory(&invalid).await.is_err());
        assert!(f.connection.stat(&invalid).await.is_err());
        assert!(f.calls().is_empty());
        let f = Fixture::new(docker, "none", "yes");
        assert!(f
            .connection
            .upload(&invalid, Path::new("/file"), Some(options()))
            .await
            .is_err());
        assert!(f
            .connection
            .download(Path::new("/file"), &invalid)
            .await
            .is_err());
        assert!(f.calls().is_empty());
    }
}

async fn invalid_ownership_preflight(docker: bool) {
    for value in ["", "bad\0value", "owner:other-group"] {
        for owner in [true, false] {
            let f = Fixture::new(docker, "none", "yes");
            let opts = if owner {
                options().with_owner(value)
            } else {
                options().with_group(value)
            };
            assert!(f
                .connection
                .upload(&f.source(), Path::new("/file"), Some(opts))
                .await
                .is_err());
            assert!(f.calls().is_empty());
        }
    }
}

async fn content_and_defaults(docker: bool) {
    let f = Fixture::new(docker, "chmod", "yes");
    assert!(f
        .connection
        .upload_content(b"synthetic", Path::new("/dir/file"), Some(options()))
        .await
        .is_err());
    assert_eq!(f.stages(), ["mkdir", "cp", "chmod"]);
    let f = Fixture::new(docker, "none", "yes");
    f.connection
        .upload(&f.source(), Path::new("/file"), None)
        .await
        .unwrap();
    assert_eq!(f.stages(), ["cp"]);
}

macro_rules! transport_tests {
    ($name:ident, $docker:expr) => {
        mod $name {
            use super::*;
            #[tokio::test]
            async fn literal_upload_arguments() {
                upload_literals($docker).await;
            }
            #[tokio::test]
            async fn mkdir_failure_stops_copy() {
                failure_stops($docker, "mkdir", &["mkdir"]).await;
            }
            #[tokio::test]
            async fn copy_failure_stops_metadata() {
                failure_stops($docker, "cp", &["mkdir", "cp"]).await;
            }
            #[tokio::test]
            async fn mode_failure_stops_ownership() {
                failure_stops($docker, "chmod", &["mkdir", "cp", "chmod"]).await;
            }
            #[tokio::test]
            async fn ownership_failure_is_not_success() {
                failure_stops($docker, "chown", &["mkdir", "cp", "chmod", "chown"]).await;
            }
            #[tokio::test]
            async fn query_arguments_and_false_controls() {
                query_literals($docker).await;
            }
            #[tokio::test]
            async fn query_execution_failures_propagate() {
                failed_queries($docker).await;
            }
            #[tokio::test]
            async fn relative_paths_are_literal_and_consistent() {
                relative_operands($docker).await;
            }
            #[tokio::test]
            async fn download_dash_is_a_file_operand() {
                download_operands($docker).await;
            }
            #[tokio::test]
            async fn invalid_paths_fail_before_cli_or_local_writes() {
                invalid_paths_preflight($docker).await;
            }
            #[tokio::test]
            async fn invalid_owner_fields_fail_before_copy() {
                invalid_ownership_preflight($docker).await;
            }
            #[tokio::test]
            async fn content_upload_and_default_options() {
                content_and_defaults($docker).await;
            }
        }
    };
}

transport_tests!(docker, true);
transport_tests!(podman, false);
