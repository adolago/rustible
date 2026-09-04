//! Capture production command strings; execute only shell builtins or an owned AWK fixture.
use super::*;
use crate::connection::{ConnectionResult, FileStat};
use std::sync::Mutex;

#[derive(Default)]
struct Capture(Mutex<Vec<String>>);

#[async_trait::async_trait]
impl Connection for Capture {
    fn identifier(&self) -> &str {
        "synthetic"
    }
    async fn is_alive(&self) -> bool {
        true
    }
    async fn execute(
        &self,
        command: &str,
        _: Option<ExecuteOptions>,
    ) -> ConnectionResult<CommandResult> {
        self.0.lock().unwrap().push(command.into());
        Ok(CommandResult {
            success: true,
            exit_code: 0,
            stdout: "synthetic-entry\n".into(),
            stderr: String::new(),
        })
    }
    async fn upload(&self, _: &Path, _: &Path, _: Option<TransferOptions>) -> ConnectionResult<()> {
        unreachable!()
    }
    async fn upload_content(
        &self,
        _: &[u8],
        _: &Path,
        _: Option<TransferOptions>,
    ) -> ConnectionResult<()> {
        unreachable!()
    }
    async fn download(&self, _: &Path, _: &Path) -> ConnectionResult<()> {
        unreachable!()
    }
    async fn download_content(&self, _: &Path) -> ConnectionResult<Vec<u8>> {
        unreachable!()
    }
    async fn path_exists(&self, _: &Path) -> ConnectionResult<bool> {
        unreachable!()
    }
    async fn is_directory(&self, _: &Path) -> ConnectionResult<bool> {
        unreachable!()
    }
    async fn stat(&self, _: &Path) -> ConnectionResult<FileStat> {
        unreachable!()
    }
    async fn close(&self) -> ConnectionResult<()> {
        Ok(())
    }
}

fn host() -> KernelDeploymentHost {
    serde_json::from_value(
        json!({"name":"synthetic", "address":"synthetic.invalid", "username":"synthetic"}),
    )
    .unwrap()
}

fn run_inner(command: &str, prelude: &str) -> std::process::Output {
    let argv = shell_words::split(command).expect("valid outer shell quoting");
    assert_eq!(&argv[..2], ["bash", "-lc"]);
    // No login/profile scripts, inherited variables or external commands. Any
    // broken quoting can reach only the explicitly defined shell builtins.
    let output = std::process::Command::new("/bin/bash")
        .args([
            "--noprofile",
            "--norc",
            "-c",
            &format!("{prelude}\n{}", argv[2]),
        ])
        .args(&argv[3..])
        .env_clear()
        .env("PATH", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success(), "captured script failed");
    output
}

fn nul_values(bytes: &[u8]) -> Vec<String> {
    assert_eq!(bytes.last(), Some(&0));
    bytes[..bytes.len() - 1]
        .split(|b| *b == 0)
        .map(|x| String::from_utf8(x.to_vec()).unwrap())
        .collect()
}

#[tokio::test]
async fn grub_setting_commands_preserve_complete_entry_arguments() {
    let prelude = r#"grub-reboot() { printf '%s\0' "$@"; }
grub2-reboot() { printf '%s\0' "$@"; }
grub-set-default() { printf '%s\0' "$@"; }
grub2-set-default() { printf '%s\0' "$@"; }"#;
    for entry in [
        "Ubuntu, with Linux 6.8.0-test",
        "entry 'quoted'",
        "entry \"double\"",
        "entry $literal",
        "entry\\literal",
        "entry; : literal",
    ] {
        let capture = Capture::default();
        let plan = BootPlan {
            bootloader: KernelBootloader::Grub,
            entry_id: entry.into(),
        };
        configure_one_shot_boot(&capture, &host(), &plan)
            .await
            .unwrap();
        commit_boot_entry(&capture, &host(), &plan).await.unwrap();
        for command in capture.0.lock().unwrap().iter() {
            let output = run_inner(command, prelude);
            assert_eq!(nul_values(&output.stdout), [entry], "{entry:?}");
        }
    }
}

#[tokio::test]
async fn systemd_setting_commands_keep_one_literal_entry() {
    for entry in ["entry with spaces", "entry'quote", "entry$literal"] {
        let capture = Capture::default();
        let plan = BootPlan {
            bootloader: KernelBootloader::SystemdBoot,
            entry_id: entry.into(),
        };
        configure_one_shot_boot(&capture, &host(), &plan)
            .await
            .unwrap();
        commit_boot_entry(&capture, &host(), &plan).await.unwrap();
        for (index, command) in capture.0.lock().unwrap().iter().enumerate() {
            let words = shell_words::split(command).unwrap();
            assert_eq!(
                words,
                [
                    "bootctl",
                    if index == 0 {
                        "set-oneshot"
                    } else {
                        "set-default"
                    },
                    entry
                ]
            );
        }
    }
}

async fn lookup(bootloader: KernelBootloader, release: &str) -> String {
    let capture = Capture::default();
    plan_boot_entry(&capture, &host(), bootloader, release)
        .await
        .unwrap();
    capture.0.into_inner().unwrap().pop().unwrap()
}

#[tokio::test]
async fn grub_lookup_preserves_release_outside_awk_program_text() {
    for release in [
        "6.8.0-test",
        "6.8 test 'quote'",
        "6.8[.]test\\literal",
        "6.8$literal",
    ] {
        let command = lookup(KernelBootloader::Grub, release).await;
        let output = run_inner(
            &command,
            r#"awk() { printf '%s\0' "$RUSTIBLE_KERNEL_RELEASE" "$@"; }"#,
        );
        let values = nul_values(&output.stdout);
        assert_eq!(values[0], release);
        assert_eq!(&values[1..3], ["-F", "'"]);
        assert!(values[3].contains("ENVIRON[\"RUSTIBLE_KERNEL_RELEASE\"]"));
        assert!(!values[3].contains(release));
    }
}

#[tokio::test]
async fn grub_lookup_awk_matches_literal_release_in_temporary_fixture() {
    let release = "6.8[.]test\\literal";
    let command = lookup(KernelBootloader::Grub, release).await;
    let output = run_inner(
        &command,
        r#"awk() { printf '%s\0' "$RUSTIBLE_KERNEL_RELEASE" "$@"; }"#,
    );
    let values = nul_values(&output.stdout);
    let directory = TempDir::new().unwrap();
    let config = directory.path().join("synthetic.cfg");
    std::fs::write(
        &config,
        format!("menuentry 'wrong 6.8.testliteral' {{\nmenuentry 'selected {release}' {{\n"),
    )
    .unwrap();
    // Use only the captured separator/program and our owned fixture, never the
    // command's real /boot paths. Environment transport preserves backslashes.
    let result = std::process::Command::new("/usr/bin/awk")
        .args(&values[1..4])
        .arg(&config)
        .env_clear()
        .env("RUSTIBLE_KERNEL_RELEASE", &values[0])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap().trim(),
        format!("selected {release}")
    );
}

#[tokio::test]
async fn systemd_lookup_passes_literal_release_to_fixed_string_grep() {
    for release in [
        "6.8.0-test",
        "6.8 test 'quote'",
        "6.8[.]test\\literal",
        "6.8$literal",
    ] {
        let command = lookup(KernelBootloader::SystemdBoot, release).await;
        let prelude = r#"set -f
[() { return 0; }
grep() { printf '%s\0' "$@" >&2; }
basename() { printf '%s\n' synthetic-entry; }"#;
        let output = run_inner(&command, prelude);
        let values = nul_values(&output.stderr);
        assert_eq!(&values[..3], ["-Fqi", "--", release]);
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "synthetic-entry\n"
        );
    }
}
