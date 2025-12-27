//! Russh Integration Tests with Homelab VMs
//!
//! These tests validate the russh connection implementation against real VMs:
//! - svr-host (192.168.178.88): Proxmox VE hypervisor
//! - svr-core (192.168.178.102): Ubuntu 24.04 services VM
//! - svr-nas (192.168.178.101): TrueNAS SCALE storage
//!
//! To run these tests:
//! ```bash
//! cargo test --test russh_homelab_tests --features russh -- --test-threads=1 --nocapture
//! ```

#![cfg(feature = "russh")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Homelab VM configuration
struct HomelabHost {
    name: &'static str,
    address: &'static str,
    user: &'static str,
}

const HOMELAB_HOSTS: &[HomelabHost] = &[
    HomelabHost {
        name: "svr-host",
        address: "192.168.178.88",
        user: "artur",
    },
    HomelabHost {
        name: "svr-core",
        address: "192.168.178.102",
        user: "artur",
    },
    HomelabHost {
        name: "svr-nas",
        address: "192.168.178.101",
        user: "artur",
    },
];

fn get_ssh_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".ssh/id_ed25519")
}

/// Test basic connectivity to all homelab hosts
#[tokio::test]
async fn test_russh_connect_to_homelab() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::Connection;

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        eprintln!("SSH key not found at {:?}, skipping test", key_path);
        return;
    }

    for host in HOMELAB_HOSTS {
        println!("Connecting to {} ({})...", host.name, host.address);

        let start = Instant::now();
        let conn = RusshConnectionBuilder::new(host.address)
            .port(22)
            .user(host.user)
            .private_key(key_path.to_string_lossy())
            .connect()
            .await;

        match conn {
            Ok(conn) => {
                let connect_time = start.elapsed();
                println!(
                    "  Connected in {:?}, identifier: {}",
                    connect_time,
                    conn.identifier()
                );

                // Test command execution
                let cmd_start = Instant::now();
                let result = conn.execute("hostname", None).await;
                let cmd_time = cmd_start.elapsed();

                match result {
                    Ok(output) => {
                        println!(
                            "  Command 'hostname' completed in {:?}: {}",
                            cmd_time,
                            output.stdout.trim()
                        );
                        assert!(output.success);
                    }
                    Err(e) => {
                        eprintln!("  Command failed: {}", e);
                    }
                }

                // Close connection
                let _ = conn.close().await;
                println!("  Connection closed");
            }
            Err(e) => {
                eprintln!("  Failed to connect to {}: {}", host.name, e);
            }
        }
    }
}

/// Test command execution with privilege escalation
#[tokio::test]
async fn test_russh_privilege_escalation() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::{Connection, ExecuteOptions};

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        eprintln!("SSH key not found, skipping test");
        return;
    }

    let host = &HOMELAB_HOSTS[1]; // svr-core

    println!("Testing privilege escalation on {}...", host.name);

    let conn = match RusshConnectionBuilder::new(host.address)
        .port(22)
        .user(host.user)
        .private_key(key_path.to_string_lossy())
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Connection failed: {}", e);
            return;
        }
    };

    // Test regular command
    let result = conn.execute("whoami", None).await.unwrap();
    println!("  Regular user: {}", result.stdout.trim());
    assert_eq!(result.stdout.trim(), host.user);

    // Test with sudo
    let options = ExecuteOptions {
        escalate: true,
        escalate_method: Some("sudo".to_string()),
        escalate_user: Some("root".to_string()),
        ..Default::default()
    };

    let result = conn.execute("whoami", Some(options)).await.unwrap();
    println!("  With sudo: {}", result.stdout.trim());
    assert_eq!(result.stdout.trim(), "root");

    let _ = conn.close().await;
}

/// Test file upload and download
#[tokio::test]
async fn test_russh_file_transfer() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::{Connection, TransferOptions};
    use std::path::Path;
    use tempfile::TempDir;

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        eprintln!("SSH key not found, skipping test");
        return;
    }

    let host = &HOMELAB_HOSTS[1]; // svr-core
    let temp_dir = TempDir::new().unwrap();

    println!("Testing file transfer on {}...", host.name);

    let conn = match RusshConnectionBuilder::new(host.address)
        .port(22)
        .user(host.user)
        .private_key(key_path.to_string_lossy())
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Connection failed: {}", e);
            return;
        }
    };

    // Create local test file
    let local_file = temp_dir.path().join("test_upload.txt");
    let test_content = format!("Rustible russh test at {:?}", Instant::now());
    std::fs::write(&local_file, &test_content).unwrap();

    // Upload file
    let remote_path = Path::new("/tmp/rustible_russh_test.txt");
    let upload_start = Instant::now();
    let upload_result = conn
        .upload(&local_file, remote_path, Some(TransferOptions::default()))
        .await;
    let upload_time = upload_start.elapsed();

    match upload_result {
        Ok(()) => {
            println!("  Upload completed in {:?}", upload_time);
        }
        Err(e) => {
            eprintln!("  Upload failed: {}", e);
            let _ = conn.close().await;
            return;
        }
    }

    // Verify file exists on remote
    let result = conn.execute("cat /tmp/rustible_russh_test.txt", None).await;
    match result {
        Ok(output) => {
            println!("  Remote content: {}", output.stdout.trim());
            assert!(output.stdout.contains("Rustible russh test"));
        }
        Err(e) => {
            eprintln!("  Verification failed: {}", e);
        }
    }

    // Download file
    let download_file = temp_dir.path().join("test_download.txt");
    let download_start = Instant::now();
    let download_result = conn.download(remote_path, &download_file).await;
    let download_time = download_start.elapsed();

    match download_result {
        Ok(()) => {
            println!("  Download completed in {:?}", download_time);
            let downloaded_content = std::fs::read_to_string(&download_file).unwrap();
            assert_eq!(downloaded_content.trim(), test_content);
        }
        Err(e) => {
            eprintln!("  Download failed: {}", e);
        }
    }

    // Cleanup
    let _ = conn
        .execute("rm -f /tmp/rustible_russh_test.txt", None)
        .await;
    let _ = conn.close().await;
}

/// Test batch command execution using parallel channel multiplexing
#[tokio::test]
async fn test_russh_execute_batch() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::Connection;

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        eprintln!("SSH key not found, skipping test");
        return;
    }

    let host = &HOMELAB_HOSTS[1]; // svr-core

    println!("Testing batch command execution on {}...", host.name);

    let conn = match RusshConnectionBuilder::new(host.address)
        .port(22)
        .user(host.user)
        .private_key(key_path.to_string_lossy())
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Connection failed: {}", e);
            return;
        }
    };

    // Test batch execution with multiple commands
    let commands: &[&str] = &["echo cmd1", "echo cmd2", "hostname", "date +%s", "whoami"];

    println!("  Executing {} commands in parallel...", commands.len());
    let start = Instant::now();
    let results = conn.execute_batch(commands, None).await;
    let batch_time = start.elapsed();

    println!("  Batch execution completed in {:?}", batch_time);

    // Verify all results
    assert_eq!(results.len(), commands.len());

    for (i, result) in results.iter().enumerate() {
        match result {
            Ok(output) => {
                println!(
                    "    Command {}: exit_code={}, stdout={}",
                    i,
                    output.exit_code,
                    output.stdout.trim()
                );
                assert!(output.success, "Command {} should succeed", i);
            }
            Err(e) => {
                panic!("Command {} failed: {}", i, e);
            }
        }
    }

    // Verify specific outputs
    assert_eq!(results[0].as_ref().unwrap().stdout.trim(), "cmd1");
    assert_eq!(results[1].as_ref().unwrap().stdout.trim(), "cmd2");
    assert_eq!(results[4].as_ref().unwrap().stdout.trim(), host.user);

    // Compare with sequential execution time
    println!("\n  Comparing with sequential execution...");
    let seq_start = Instant::now();
    for cmd in commands {
        let _ = conn.execute(cmd, None).await;
    }
    let seq_time = seq_start.elapsed();

    println!("  Sequential execution time: {:?}", seq_time);
    println!(
        "  Speedup: {:.2}x",
        seq_time.as_secs_f64() / batch_time.as_secs_f64()
    );

    let _ = conn.close().await;
}

/// Benchmark russh connection and command execution
#[tokio::test]
async fn benchmark_russh_performance() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::Connection;

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        eprintln!("SSH key not found, skipping benchmark");
        return;
    }

    let host = &HOMELAB_HOSTS[1]; // svr-core
    let iterations = 10;

    println!(
        "Benchmarking russh on {} ({} iterations)...",
        host.name, iterations
    );

    // Measure connection time
    let mut connect_times = vec![];
    let mut command_times = vec![];

    for i in 0..iterations {
        let connect_start = Instant::now();
        let conn = RusshConnectionBuilder::new(host.address)
            .port(22)
            .user(host.user)
            .private_key(key_path.to_string_lossy())
            .connect()
            .await;

        match conn {
            Ok(conn) => {
                connect_times.push(connect_start.elapsed());

                // Run a simple command
                let cmd_start = Instant::now();
                let _ = conn.execute("echo hello", None).await;
                command_times.push(cmd_start.elapsed());

                let _ = conn.close().await;
            }
            Err(e) => {
                eprintln!("  Iteration {} failed: {}", i, e);
            }
        }
    }

    if !connect_times.is_empty() {
        let avg_connect: Duration =
            connect_times.iter().sum::<Duration>() / connect_times.len() as u32;
        let avg_command: Duration =
            command_times.iter().sum::<Duration>() / command_times.len() as u32;
        let min_connect = connect_times.iter().min().unwrap();
        let max_connect = connect_times.iter().max().unwrap();
        let min_command = command_times.iter().min().unwrap();
        let max_command = command_times.iter().max().unwrap();

        println!("Results:");
        println!(
            "  Connect time: avg={:?}, min={:?}, max={:?}",
            avg_connect, min_connect, max_connect
        );
        println!(
            "  Command time: avg={:?}, min={:?}, max={:?}",
            avg_command, min_command, max_command
        );
    }
}
