//! SSH Benchmark: russh vs ssh2
//!
//! Compares performance of russh (async, pure Rust) vs ssh2 (sync, libssh2)
//!
//! Run with both SSH backends enabled:
//! ```bash
//! cargo test --test ssh_benchmark --features "russh,ssh2-backend" -- --test-threads=1 --nocapture
//! ```
//!
//! NOTE: This benchmark requires both russh and ssh2-backend features.

#![cfg(all(feature = "russh", feature = "ssh2-backend"))]

use std::path::PathBuf;
use std::time::{Duration, Instant};

const HOST: &str = "192.168.178.102"; // svr-core
const USER: &str = "artur";
const ITERATIONS: usize = 20;

fn get_ssh_key_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".ssh/id_ed25519")
}

#[derive(Debug, Default)]
struct BenchResult {
    connect_times: Vec<Duration>,
    command_times: Vec<Duration>,
    failures: usize,
}

impl BenchResult {
    fn summary(&self) -> String {
        if self.connect_times.is_empty() {
            return format!("No successful runs ({} failures)", self.failures);
        }

        let avg_connect: Duration =
            self.connect_times.iter().sum::<Duration>() / self.connect_times.len() as u32;
        let avg_command: Duration =
            self.command_times.iter().sum::<Duration>() / self.command_times.len() as u32;
        let min_connect = self.connect_times.iter().min().unwrap();
        let max_connect = self.connect_times.iter().max().unwrap();
        let min_command = self.command_times.iter().min().unwrap();
        let max_command = self.command_times.iter().max().unwrap();

        format!(
            "Connect: avg={:?} min={:?} max={:?} | Command: avg={:?} min={:?} max={:?} | Failures: {}",
            avg_connect, min_connect, max_connect,
            avg_command, min_command, max_command,
            self.failures
        )
    }
}

/// Benchmark ssh2 connection
async fn bench_ssh2(iterations: usize) -> BenchResult {
    use rustible::connection::ssh::SshConnection;
    use rustible::connection::{Connection, ConnectionConfig};

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        return BenchResult {
            failures: iterations,
            ..Default::default()
        };
    }

    let mut result = BenchResult::default();
    let config = ConnectionConfig::default();

    for _ in 0..iterations {
        let connect_start = Instant::now();

        match SshConnection::connect(HOST, 22, USER, None, &config).await {
            Ok(conn) => {
                result.connect_times.push(connect_start.elapsed());

                let cmd_start = Instant::now();
                let cmd_result = conn.execute("echo hello", None).await;

                match cmd_result {
                    Ok(_) => {
                        result.command_times.push(cmd_start.elapsed());
                    }
                    Err(_) => {
                        result.failures += 1;
                    }
                }

                let _ = conn.close().await;
            }
            Err(_) => {
                result.failures += 1;
            }
        }
    }

    result
}

/// Benchmark russh connection
#[cfg(feature = "russh")]
async fn bench_russh(iterations: usize) -> BenchResult {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::Connection;

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        return BenchResult {
            failures: iterations,
            ..Default::default()
        };
    }

    let mut result = BenchResult::default();

    for _ in 0..iterations {
        let connect_start = Instant::now();

        let conn = RusshConnectionBuilder::new(HOST)
            .port(22)
            .user(USER)
            .private_key(key_path.to_string_lossy())
            .connect()
            .await;

        match conn {
            Ok(conn) => {
                result.connect_times.push(connect_start.elapsed());

                let cmd_start = Instant::now();
                let cmd_result = conn.execute("echo hello", None).await;

                match cmd_result {
                    Ok(_) => {
                        result.command_times.push(cmd_start.elapsed());
                    }
                    Err(_) => {
                        result.failures += 1;
                    }
                }

                let _ = conn.close().await;
            }
            Err(_) => {
                result.failures += 1;
            }
        }
    }

    result
}

/// Compare ssh2 vs russh performance
#[tokio::test]
#[cfg(feature = "russh")]
async fn compare_ssh_performance() {
    println!("\n=== SSH Performance Comparison ===");
    println!("Host: {}@{}", USER, HOST);
    println!("Iterations: {}\n", ITERATIONS);

    // Benchmark ssh2
    println!("Benchmarking ssh2 (sync, libssh2)...");
    let ssh2_result = bench_ssh2(ITERATIONS).await;
    println!("  ssh2: {}", ssh2_result.summary());

    // Benchmark russh
    println!("\nBenchmarking russh (async, pure Rust)...");
    let russh_result = bench_russh(ITERATIONS).await;
    println!("  russh: {}", russh_result.summary());

    // Calculate comparison
    if !ssh2_result.connect_times.is_empty() && !russh_result.connect_times.is_empty() {
        let ssh2_avg_connect: Duration = ssh2_result.connect_times.iter().sum::<Duration>()
            / ssh2_result.connect_times.len() as u32;
        let russh_avg_connect: Duration = russh_result.connect_times.iter().sum::<Duration>()
            / russh_result.connect_times.len() as u32;

        let ssh2_avg_cmd: Duration = ssh2_result.command_times.iter().sum::<Duration>()
            / ssh2_result.command_times.len() as u32;
        let russh_avg_cmd: Duration = russh_result.command_times.iter().sum::<Duration>()
            / russh_result.command_times.len() as u32;

        let connect_ratio =
            ssh2_avg_connect.as_nanos() as f64 / russh_avg_connect.as_nanos() as f64;
        let cmd_ratio = ssh2_avg_cmd.as_nanos() as f64 / russh_avg_cmd.as_nanos() as f64;

        println!("\n=== Comparison ===");
        println!(
            "Connect time: russh is {:.2}x {} than ssh2",
            if connect_ratio > 1.0 {
                connect_ratio
            } else {
                1.0 / connect_ratio
            },
            if connect_ratio > 1.0 {
                "faster"
            } else {
                "slower"
            }
        );
        println!(
            "Command time: russh is {:.2}x {} than ssh2",
            if cmd_ratio > 1.0 {
                cmd_ratio
            } else {
                1.0 / cmd_ratio
            },
            if cmd_ratio > 1.0 { "faster" } else { "slower" }
        );
    }
}

/// Benchmark parallel command execution
#[tokio::test]
#[cfg(feature = "russh")]
async fn compare_parallel_performance() {
    use rustible::connection::russh::RusshConnectionBuilder;
    use rustible::connection::ssh::SshConnection;
    use rustible::connection::{Connection, ConnectionConfig};

    let key_path = get_ssh_key_path();
    if !key_path.exists() {
        println!("SSH key not found, skipping parallel benchmark");
        return;
    }

    let parallel_count = 5;
    let commands_per_conn = 10;

    println!("\n=== Parallel Command Benchmark ===");
    println!(
        "Connections: {}, Commands per connection: {}",
        parallel_count, commands_per_conn
    );

    // Benchmark russh parallel
    println!("\nBenchmarking russh parallel execution...");
    let russh_start = Instant::now();

    let mut handles = vec![];
    for _ in 0..parallel_count {
        let key = key_path.clone();
        let handle = tokio::spawn(async move {
            let conn = RusshConnectionBuilder::new(HOST)
                .port(22)
                .user(USER)
                .private_key(key.to_string_lossy())
                .connect()
                .await
                .ok()?;

            for _ in 0..commands_per_conn {
                conn.execute("echo hello", None).await.ok()?;
            }

            conn.close().await.ok();
            Some(())
        });
        handles.push(handle);
    }

    let mut russh_success = 0;
    for handle in handles {
        if handle.await.unwrap().is_some() {
            russh_success += 1;
        }
    }
    let russh_time = russh_start.elapsed();
    println!(
        "  russh: {} connections, {} total commands in {:?}",
        russh_success,
        russh_success * commands_per_conn,
        russh_time
    );

    // Benchmark ssh2 parallel
    println!("\nBenchmarking ssh2 parallel execution...");
    let ssh2_start = Instant::now();

    let mut ssh2_handles = vec![];
    for _ in 0..parallel_count {
        let handle = tokio::spawn(async move {
            let config = ConnectionConfig::default();
            let conn = SshConnection::connect(HOST, 22, USER, None, &config)
                .await
                .ok()?;

            for _ in 0..commands_per_conn {
                conn.execute("echo hello", None).await.ok()?;
            }

            conn.close().await.ok();
            Some(())
        });
        ssh2_handles.push(handle);
    }

    let mut ssh2_success = 0;
    for handle in ssh2_handles {
        if handle.await.unwrap().is_some() {
            ssh2_success += 1;
        }
    }
    let ssh2_time = ssh2_start.elapsed();
    println!(
        "  ssh2: {} connections, {} total commands in {:?}",
        ssh2_success,
        ssh2_success * commands_per_conn,
        ssh2_time
    );

    // Comparison
    if russh_time.as_nanos() > 0 && ssh2_time.as_nanos() > 0 {
        let ratio = ssh2_time.as_nanos() as f64 / russh_time.as_nanos() as f64;
        println!("\n=== Parallel Comparison ===");
        println!(
            "russh is {:.2}x {} than ssh2 for parallel workloads",
            if ratio > 1.0 { ratio } else { 1.0 / ratio },
            if ratio > 1.0 { "faster" } else { "slower" }
        );
    }
}
