//! Bounded regressions: only private temporary files and loopback HTTP.
#![cfg(unix)]

use rustible::modules::{Module, ModuleContext, ModuleParams};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

fn params(values: serde_json::Value) -> ModuleParams {
    serde_json::from_value(values).unwrap()
}

#[test]
fn diligence_copy_directory_child() {
    if std::env::var("RUSTIBLE_DILIGENCE_CHILD").as_deref() != Ok("directories") {
        return;
    }
    rustible::modules::copy::CopyModule.execute(
        &params(serde_json::json!({"content":"benign", "dest":"existing/new/deep/file", "directory_mode":"0755"})),
        &ModuleContext::default(),
    ).unwrap();
}

#[test]
fn diligence_copy_preserves_existing_ancestor_modes() {
    let scratch = tempfile::tempdir().unwrap();
    fs::create_dir(scratch.path().join("existing")).unwrap();
    fs::set_permissions(
        scratch.path().join("existing"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "diligence_copy_directory_child", "--nocapture"])
        .env("RUSTIBLE_DILIGENCE_CHILD", "directories")
        .current_dir(scratch.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::metadata(scratch.path().join("existing"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o700
    );
    assert_eq!(
        fs::metadata(scratch.path().join("existing/new/deep"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o755
    );
}

#[test]
fn diligence_permission_writer_child() {
    let Ok(kind) = std::env::var("RUSTIBLE_DILIGENCE_WRITER") else {
        return;
    };
    let mode = std::env::var("RUSTIBLE_DILIGENCE_MODE").unwrap();
    let numeric_mode = u32::from_str_radix(&mode, 8).unwrap();
    if kind == "utility" {
        rustible::utils::secure_write_file(
            Path::new("fifo"),
            &"x".repeat(1024 * 1024),
            false,
            Some(numeric_mode),
        )
        .unwrap();
    } else {
        rustible::modules::copy::CopyModule
            .execute(
                &params(serde_json::json!({"src":"source", "dest":"fifo", "mode":mode})),
                &ModuleContext::default(),
            )
            .unwrap();
    }
}

#[test]
fn diligence_permissions_restrict_before_first_content_byte() {
    let mut observations = Vec::new();
    for (kind, old_mode, target_mode, expected) in [
        ("utility", 0o644, 0o600, 0o600),
        ("copy", 0o644, 0o600, 0o600),
        ("utility", 0o600, 0o644, 0o600),
        ("copy", 0o600, 0o644, 0o600),
        ("utility", 0o755, 0o4755, 0o755),
        ("copy", 0o755, 0o4755, 0o755),
    ] {
        let scratch = tempfile::tempdir().unwrap();
        let fifo = scratch.path().join("fifo");
        nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::from_bits_truncate(0o644)).unwrap();
        fs::set_permissions(&fifo, fs::Permissions::from_mode(old_mode)).unwrap();
        fs::write(scratch.path().join("source"), vec![b'x'; 1024 * 1024]).unwrap();
        let mut reader = fs::OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NONBLOCK)
            .open(&fifo)
            .unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "diligence_permission_writer_child"])
            .env("RUSTIBLE_DILIGENCE_WRITER", kind)
            .env("RUSTIBLE_DILIGENCE_MODE", format!("0{target_mode:o}"))
            .current_dir(scratch.path())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed = false;
        while Instant::now() < deadline {
            if matches!(reader.read(&mut [0u8; 1]), Ok(1)) {
                observed = true;
                break;
            }
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let mode = fs::metadata(&fifo).unwrap().permissions().mode() & 0o7777;
        let _ = child.kill();
        child.wait().unwrap();
        observations.push((kind, observed, mode, expected));
    }
    for (kind, observed, mode, expected) in observations {
        assert!(
            observed,
            "{kind}: writer did not deliver a byte within five seconds"
        );
        assert_eq!(
            mode, expected,
            "{kind}: only restrict permissions until replacement is complete"
        );
    }
}

#[test]
fn diligence_unarchive_marker_never_overwrites_sibling() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("extracted");
    let sentinel = scratch.path().join("sentinel");
    fs::write(&sentinel, "unchanged").unwrap();
    let archive = scratch.path().join("input.tar");
    let mut builder = tar::Builder::new(fs::File::create(&archive).unwrap());
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Symlink);
    header.set_path(".unarchive_marker").unwrap();
    header.set_link_name("../sentinel").unwrap();
    header.set_size(0);
    header.set_mode(0o777);
    header.set_cksum();
    builder.append(&header, std::io::empty()).unwrap();
    builder.finish().unwrap();
    drop(builder);
    let _ = rustible::modules::unarchive::UnarchiveModule.execute(
        &params(serde_json::json!({"src":archive, "dest":dest})),
        &ModuleContext::default(),
    );
    assert_eq!(fs::read_to_string(sentinel).unwrap(), "unchanged");
}

#[test]
fn diligence_zip_never_follows_existing_parent_or_file_links() {
    let mut observations = Vec::new();
    for parent_link in [false, true] {
        let scratch = tempfile::tempdir().unwrap();
        let dest = scratch.path().join("extracted");
        let outside = scratch.path().join("outside");
        fs::create_dir(&dest).unwrap();
        fs::create_dir(&outside).unwrap();
        let sentinel = outside.join("file");
        fs::write(&sentinel, "unchanged").unwrap();
        if parent_link {
            std::os::unix::fs::symlink(&outside, dest.join("parent")).unwrap();
        } else {
            fs::create_dir(dest.join("parent")).unwrap();
            std::os::unix::fs::symlink(&sentinel, dest.join("parent/file")).unwrap();
        }
        let archive = scratch.path().join("input.zip");
        let mut builder = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
        builder
            .start_file("parent/file", zip::write::SimpleFileOptions::default())
            .unwrap();
        builder.write_all(b"replacement").unwrap();
        builder.finish().unwrap();
        let _ = rustible::modules::unarchive::UnarchiveModule.execute(
            &params(serde_json::json!({"src":archive, "dest":dest})),
            &ModuleContext::default(),
        );
        observations.push((parent_link, fs::read_to_string(sentinel).unwrap()));
    }
    for (parent_link, content) in observations {
        assert_eq!(content, "unchanged", "parent_link={parent_link}");
    }
}

#[test]
fn diligence_archive_rejects_destination_inside_removed_source() {
    let mut observations = Vec::new();
    for format in ["tar", "gz", "zip"] {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("sentinel"), "unchanged").unwrap();
        let result = rustible::modules::archive::ArchiveModule.execute(
            &params(serde_json::json!({"path":source, "dest":source.join("result"), "format":format, "remove":true})),
            &ModuleContext::default());
        observations.push((
            format,
            result.is_err(),
            fs::read_to_string(source.join("sentinel")).ok(),
        ));
    }
    for (format, rejected, content) in observations {
        assert!(rejected, "{format}: destructive overlap must be rejected");
        assert_eq!(content.as_deref(), Some("unchanged"), "{format}");
    }
}

#[derive(Default)]
struct UploadModeProbe {
    mode: std::sync::Mutex<Option<Option<u32>>>,
    commands: std::sync::Mutex<Vec<String>>,
    allow_upload: bool,
    fail_chmod: bool,
}

fn unused_connection_call<T>() -> rustible::connection::ConnectionResult<T> {
    Err(rustible::connection::ConnectionError::TransferFailed(
        "synthetic probe stops before remote execution".into(),
    ))
}

#[async_trait::async_trait]
impl rustible::connection::Connection for UploadModeProbe {
    fn identifier(&self) -> &str {
        "synthetic-probe"
    }
    async fn is_alive(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        command: &str,
        _: Option<rustible::connection::ExecuteOptions>,
    ) -> rustible::connection::ConnectionResult<rustible::connection::CommandResult> {
        self.commands.lock().unwrap().push(command.into());
        if command.starts_with("chmod ") && self.fail_chmod {
            Ok(rustible::connection::CommandResult {
                stdout: String::new(),
                stderr: "synthetic chmod failure".into(),
                exit_code: 1,
                success: false,
            })
        } else if command.starts_with("rm ") || command.starts_with("chmod ") {
            Ok(rustible::connection::CommandResult::success(
                String::new(),
                String::new(),
            ))
        } else {
            unused_connection_call()
        }
    }
    async fn upload(
        &self,
        _: &Path,
        _: &Path,
        _: Option<rustible::connection::TransferOptions>,
    ) -> rustible::connection::ConnectionResult<()> {
        unused_connection_call()
    }
    async fn upload_content(
        &self,
        _: &[u8],
        _: &Path,
        options: Option<rustible::connection::TransferOptions>,
    ) -> rustible::connection::ConnectionResult<()> {
        *self.mode.lock().unwrap() = Some(options.and_then(|options| options.mode));
        if self.allow_upload {
            Ok(())
        } else {
            unused_connection_call()
        }
    }
    async fn download(&self, _: &Path, _: &Path) -> rustible::connection::ConnectionResult<()> {
        unused_connection_call()
    }
    async fn download_content(&self, _: &Path) -> rustible::connection::ConnectionResult<Vec<u8>> {
        unused_connection_call()
    }
    async fn path_exists(&self, _: &Path) -> rustible::connection::ConnectionResult<bool> {
        unused_connection_call()
    }
    async fn is_directory(&self, _: &Path) -> rustible::connection::ConnectionResult<bool> {
        unused_connection_call()
    }
    async fn stat(
        &self,
        _: &Path,
    ) -> rustible::connection::ConnectionResult<rustible::connection::FileStat> {
        unused_connection_call()
    }
    async fn close(&self) -> rustible::connection::ConnectionResult<()> {
        Ok(())
    }
}

#[test]
fn diligence_script_requests_private_mode_before_upload() {
    let scratch = tempfile::tempdir().unwrap();
    let script = scratch.path().join("benign.sh");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    let probe = std::sync::Arc::new(UploadModeProbe::default());
    let context = ModuleContext {
        connection: Some(probe.clone()),
        ..Default::default()
    };
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    assert!(rustible::modules::script::ScriptModule
        .execute(&params(serde_json::json!({"script":script})), &context)
        .is_err());
    assert_eq!(*probe.mode.lock().unwrap(), Some(Some(0o700)));
}

#[test]
fn diligence_script_checks_chmod_and_cleans_up_after_failures() {
    for fail_chmod in [false, true] {
        let scratch = tempfile::tempdir().unwrap();
        let script = scratch.path().join("benign.sh");
        fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
        let probe = std::sync::Arc::new(UploadModeProbe {
            allow_upload: true,
            fail_chmod,
            ..Default::default()
        });
        let context = ModuleContext {
            connection: Some(probe.clone()),
            ..Default::default()
        };
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _entered = runtime.enter();
        assert!(rustible::modules::script::ScriptModule
            .execute(&params(serde_json::json!({"script":script})), &context)
            .is_err());
        let commands = probe.commands.lock().unwrap();
        assert!(commands.last().unwrap().starts_with("rm -f -- "));
        assert_eq!(commands.len(), if fail_chmod { 2 } else { 3 });
    }
}

#[test]
fn diligence_archive_rejects_source_aliases_before_truncation() {
    for alias in ["same", "symlink", "hardlink"] {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        fs::write(&source, "unchanged").unwrap();
        let dest = if alias == "same" {
            source.clone()
        } else {
            scratch.path().join("alias")
        };
        if alias == "symlink" {
            std::os::unix::fs::symlink(&source, &dest).unwrap();
        }
        if alias == "hardlink" {
            fs::hard_link(&source, &dest).unwrap();
        }
        assert!(rustible::modules::archive::ArchiveModule
            .execute(
                &params(
                    serde_json::json!({"path":source,"dest":dest,"format":"tar","remove":true})
                ),
                &ModuleContext::default()
            )
            .is_err());
        assert_eq!(fs::read_to_string(source).unwrap(), "unchanged", "{alias}");
    }
}

#[test]
fn diligence_archive_member_hardlink_child() {
    let Ok(format) = std::env::var("RUSTIBLE_DILIGENCE_ARCHIVE_ALIAS") else {
        return;
    };
    rustible::modules::archive::ArchiveModule
        .execute(
            &params(serde_json::json!({"path":"source","dest":"archive","format":format})),
            &ModuleContext::default(),
        )
        .unwrap();
}

#[test]
fn diligence_archive_output_hardlink_never_truncates_source_member() {
    for format in ["tar", "gz", "zip"] {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        fs::create_dir(&source).unwrap();
        let member = source.join("member");
        fs::write(&member, "unchanged").unwrap();
        let archive = scratch.path().join("archive");
        fs::hard_link(&member, &archive).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "diligence_archive_member_hardlink_child"])
            .env("RUSTIBLE_DILIGENCE_ARCHIVE_ALIAS", format)
            .current_dir(scratch.path());
        // A broken self-copy can grow forever. Restrict the child before exec;
        // the parent owns the TempDir and always reaps it before assertions.
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                for (resource, limit) in [
                    (nix::libc::RLIMIT_FSIZE, 1024 * 1024),
                    (nix::libc::RLIMIT_AS, 256 * 1024 * 1024),
                    (nix::libc::RLIMIT_CPU, 2),
                ] {
                    let limits = nix::libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if nix::libc::setrlimit(resource, &limits) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                break child.wait().unwrap();
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(fs::metadata(&member).unwrap().len(), 9, "{format}");
        assert_eq!(
            fs::read_to_string(&member).unwrap(),
            "unchanged",
            "{format}"
        );
        assert!(status.success(), "{format}: {status}");
        let restored = scratch.path().join("restored");
        rustible::modules::unarchive::UnarchiveModule
            .execute(
                &params(serde_json::json!({"src":archive,"dest":restored,"format":format})),
                &ModuleContext::default(),
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(restored.join("member")).unwrap(),
            "unchanged"
        );
    }
}

#[test]
fn diligence_single_file_archive_round_trip_and_remove() {
    for format in ["tar", "gz", "zip"] {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source.bin");
        let archive = scratch.path().join("archive");
        let dest = scratch.path().join("restored");
        let content = [0u8, 0xff, 1, 2, 3];
        fs::write(&source, content).unwrap();
        rustible::modules::archive::ArchiveModule
            .execute(
                &params(
                    serde_json::json!({"path":source,"dest":archive,"format":format,"remove":true}),
                ),
                &ModuleContext::default(),
            )
            .unwrap();
        assert!(!source.exists());
        rustible::modules::unarchive::UnarchiveModule
            .execute(
                &params(serde_json::json!({"src":archive,"dest":dest,"format":format})),
                &ModuleContext::default(),
            )
            .unwrap();
        assert_eq!(
            fs::read(dest.join("source.bin")).unwrap(),
            content,
            "{format}"
        );
    }
}

#[test]
fn diligence_archive_publication_modes_and_failure_preserve_source() {
    for format in ["tar", "gz", "zip"] {
        let scratch = tempfile::tempdir().unwrap();
        let source = scratch.path().join("source");
        let archive = scratch.path().join("archive");
        fs::write(&source, "unchanged").unwrap();
        for expected_mode in [0o600, 0o640] {
            if archive.exists() {
                fs::set_permissions(&archive, fs::Permissions::from_mode(expected_mode)).unwrap();
            }
            rustible::modules::archive::ArchiveModule
                .execute(
                    &params(serde_json::json!({"path":source,"dest":archive,"format":format})),
                    &ModuleContext::default(),
                )
                .unwrap();
            assert_eq!(
                fs::metadata(&archive).unwrap().permissions().mode() & 0o7777,
                expected_mode
            );
        }
        let protected = scratch.path().join("protected");
        fs::create_dir(&protected).unwrap();
        fs::write(protected.join("sentinel"), "unchanged").unwrap();
        assert!(rustible::modules::archive::ArchiveModule.execute(
            &params(serde_json::json!({"path":source,"dest":protected,"format":format,"remove":true})),
            &ModuleContext::default(),
        ).is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), "unchanged");
        assert_eq!(
            fs::read_to_string(protected.join("sentinel")).unwrap(),
            "unchanged"
        );
        assert_eq!(
            fs::read_dir(scratch.path()).unwrap().count(),
            3,
            "staged output leaked"
        );
    }
}

#[test]
fn diligence_replacements_truncate_and_restore_final_modes() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("dest");
    let source = scratch.path().join("source");
    fs::write(&source, "short").unwrap();
    for kind in ["utility", "copy"] {
        fs::write(&dest, "previously much longer contents").unwrap();
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o600)).unwrap();
        if kind == "utility" {
            rustible::utils::secure_write_file(&dest, "short", false, Some(0o4750)).unwrap();
        } else {
            rustible::modules::copy::CopyModule
                .execute(
                    &params(serde_json::json!({"src":source,"dest":dest,"mode":"04750"})),
                    &ModuleContext::default(),
                )
                .unwrap();
        }
        assert_eq!(fs::read_to_string(&dest).unwrap(), "short", "{kind}");
        assert_eq!(
            fs::metadata(&dest).unwrap().permissions().mode() & 0o7777,
            0o4750,
            "{kind}"
        );
    }
}

fn http_stub(body_size: usize, chunked: bool) -> (String, std::thread::JoinHandle<()>) {
    http_stub_header(body_size, chunked, None)
}

fn http_stub_header(
    body_size: usize,
    chunked: bool,
    required_header: Option<&'static str>,
) -> (String, std::thread::JoinHandle<()>) {
    http_stub_custom(body_size, chunked, required_header, None)
}

fn http_stub_custom(
    body_size: usize,
    chunked: bool,
    required_header: Option<&'static str>,
    concurrent_destination: Option<std::path::PathBuf>,
) -> (String, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (mut stream, _) = loop {
            match listener.accept() {
                Ok(connection) => break connection,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(_) => return,
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            if stream.read_exact(&mut byte).is_err() {
                return;
            }
            request.push(byte[0]);
            if request.len() > 16384 {
                return;
            }
        }
        if let Some(destination) = concurrent_destination {
            fs::write(destination, "concurrent creator").unwrap();
        }
        if let Some(header) = required_header {
            if !String::from_utf8_lossy(&request)
                .to_ascii_lowercase()
                .contains(header)
            {
                let _ = stream.write_all(
                    b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
                return;
            }
        }
        let headers = if chunked {
            "Transfer-Encoding: chunked".to_string()
        } else {
            format!("Content-Length: {body_size}")
        };
        if write!(
            stream,
            "HTTP/1.1 200 OK\r\n{headers}\r\nConnection: close\r\n\r\n"
        )
        .is_err()
        {
            return;
        }
        let block = [b'x'; 8192];
        let mut remaining = body_size;
        while remaining != 0 {
            let count = remaining.min(block.len());
            if chunked && write!(stream, "{count:x}\r\n").is_err() {
                return;
            }
            if stream.write_all(&block[..count]).is_err() {
                return;
            }
            if chunked && stream.write_all(b"\r\n").is_err() {
                return;
            }
            remaining -= count;
        }
        if chunked {
            let _ = stream.write_all(b"0\r\n\r\n");
        }
    });
    (format!("http://{address}/file"), thread)
}

#[test]
fn diligence_get_url_writes_local_bytes_with_requested_mode() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("download");
    let (url, server) = http_stub(17, false);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let result = rustible::modules::get_url::GetUrlModule.execute(
        &params(serde_json::json!({"url":url, "dest":dest, "mode":"0600", "timeout":5})),
        &ModuleContext::default(),
    );
    server.join().unwrap();
    assert!(result.unwrap().changed);
    assert_eq!(fs::read(&dest).unwrap(), vec![b'x'; 17]);
    assert_eq!(
        fs::metadata(dest).unwrap().permissions().mode() & 0o7777,
        0o600
    );
}

#[test]
fn diligence_get_url_rejects_oversized_chunked_response() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("download");
    // Existing public limit is 100MiB; server allocates only an 8KiB block.
    let (url, server) = http_stub(100 * 1024 * 1024 + 1, true);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let result = rustible::modules::get_url::GetUrlModule.execute(
        &params(serde_json::json!({"url":url, "dest":dest, "timeout":10})),
        &ModuleContext::default(),
    );
    server.join().unwrap();
    assert!(result.is_err());
    assert!(!dest.exists());
}

#[test]
fn diligence_get_url_applies_custom_headers() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("download");
    let (url, server) = http_stub_header(3, false, Some("x-diligence: synthetic-proof"));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let result = rustible::modules::get_url::GetUrlModule.execute(
        &params(serde_json::json!({"url":url,"dest":dest,"headers":{"X-Diligence":"synthetic-proof"},"timeout":5})), &ModuleContext::default());
    server.join().unwrap();
    assert!(result.unwrap().changed);
    assert_eq!(fs::read(dest).unwrap(), b"xxx");
}

#[test]
fn diligence_get_url_preserves_existing_file_on_skip_and_checksum_failure() {
    let scratch = tempfile::tempdir().unwrap();
    let dest = scratch.path().join("existing");
    fs::write(&dest, "unchanged").unwrap();
    fs::set_permissions(&dest, fs::Permissions::from_mode(0o640)).unwrap();
    // Port zero cannot address a listening service; this path must not request it.
    let skipped = rustible::modules::get_url::GetUrlModule
        .execute(
            &params(serde_json::json!({"url":"http://127.0.0.1:0/not-requested","dest":dest})),
            &ModuleContext::default(),
        )
        .unwrap();
    assert!(!skipped.changed);
    let (url, server) = http_stub(3, false);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _entered = runtime.enter();
    let failed = rustible::modules::get_url::GetUrlModule.execute(
        &params(serde_json::json!({"url":url,"dest":dest,"force":true,"checksum":"sha256:00","mode":"0600","timeout":5})), &ModuleContext::default());
    server.join().unwrap();
    assert!(failed.is_err());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "unchanged");
    assert_eq!(
        fs::metadata(dest).unwrap().permissions().mode() & 0o7777,
        0o640
    );
}

#[test]
fn diligence_get_url_does_not_clobber_concurrent_creator_without_force() {
    for force in [false, true] {
        let scratch = tempfile::tempdir().unwrap();
        let dest = scratch.path().join("download");
        let (url, server) = http_stub_custom(3, false, None, Some(dest.clone()));
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _entered = runtime.enter();
        let result = rustible::modules::get_url::GetUrlModule.execute(
            &params(serde_json::json!({"url":url,"dest":dest,"force":force,"timeout":5})),
            &ModuleContext::default(),
        );
        server.join().unwrap();
        assert_eq!(result.unwrap().changed, force);
        assert_eq!(
            fs::read_to_string(dest).unwrap(),
            if force { "xxx" } else { "concurrent creator" }
        );
    }
}
