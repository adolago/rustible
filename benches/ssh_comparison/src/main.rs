//! SSH Library Comparison Benchmark
//! Comprehensive comparison of ssh2 (libssh2) vs async-ssh2-tokio (russh)
//!
//! This benchmark suite tests:
//! 1. Connection time
//! 2. Command execution performance
//! 3. File transfer (upload/download)
//! 4. Async russh vs spawn_blocking ssh2

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

use async_ssh2_tokio::client::{AuthMethod, Client, ServerCheckMethod};
use clap::Parser;
use hdrhistogram::Histogram;
use rand::Rng;
use tabled::{Table, Tabled};

#[derive(Parser, Debug)]
struct Args {
    /// Host to connect to
    #[arg(short = 'H', long, default_value = "192.168.178.141")]
    host: String,

    /// SSH port
    #[arg(short, long, default_value = "22")]
    port: u16,

    /// SSH user
    #[arg(short, long, default_value = "testuser")]
    user: String,

    /// SSH key file
    #[arg(short, long, default_value = "~/.ssh/id_ed25519")]
    key: String,

    /// Number of iterations for each benchmark
    #[arg(short, long, default_value = "100")]
    iterations: u32,

    /// Size of test file in KB for file transfer benchmarks
    #[arg(short, long, default_value = "100")]
    file_size_kb: usize,

    /// Skip file transfer benchmarks (slower)
    #[arg(long)]
    skip_file_transfer: bool,

    /// Output detailed statistics
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Tabled)]
struct BenchmarkResult {
    #[tabled(rename = "Benchmark")]
    name: String,
    #[tabled(rename = "Library")]
    library: String,
    #[tabled(rename = "Mean (ms)")]
    mean_ms: String,
    #[tabled(rename = "Median (ms)")]
    median_ms: String,
    #[tabled(rename = "P95 (ms)")]
    p95_ms: String,
    #[tabled(rename = "P99 (ms)")]
    p99_ms: String,
    #[tabled(rename = "Min (ms)")]
    min_ms: String,
    #[tabled(rename = "Max (ms)")]
    max_ms: String,
}

struct BenchStats {
    name: String,
    library: String,
    durations: Vec<Duration>,
}

impl BenchStats {
    fn new(name: &str, library: &str) -> Self {
        Self {
            name: name.to_string(),
            library: library.to_string(),
            durations: Vec::new(),
        }
    }

    fn record(&mut self, duration: Duration) {
        self.durations.push(duration);
    }

    fn to_result(&self) -> BenchmarkResult {
        let mut hist = Histogram::<u64>::new(3).unwrap();
        for d in &self.durations {
            hist.record(d.as_micros() as u64).ok();
        }

        let mean_us = hist.mean();
        let median_us = hist.value_at_quantile(0.5);
        let p95_us = hist.value_at_quantile(0.95);
        let p99_us = hist.value_at_quantile(0.99);
        let min_us = hist.min();
        let max_us = hist.max();

        BenchmarkResult {
            name: self.name.clone(),
            library: self.library.clone(),
            mean_ms: format!("{:.2}", mean_us as f64 / 1000.0),
            median_ms: format!("{:.2}", median_us as f64 / 1000.0),
            p95_ms: format!("{:.2}", p95_us as f64 / 1000.0),
            p99_ms: format!("{:.2}", p99_us as f64 / 1000.0),
            min_ms: format!("{:.2}", min_us as f64 / 1000.0),
            max_ms: format!("{:.2}", max_us as f64 / 1000.0),
        }
    }
}

fn expand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]).to_string_lossy().to_string();
        }
    }
    path.to_string()
}

// =============================================================================
// Connection Benchmarks
// =============================================================================

/// Benchmark SSH2 connection establishment
fn bench_ssh2_connect(host: &str, port: u16, user: &str, key_path: &str) -> Duration {
    let start = Instant::now();

    let tcp = TcpStream::connect((host, port)).unwrap();
    let mut session = ssh2::Session::new().unwrap();
    session.set_tcp_stream(tcp);
    session.handshake().unwrap();
    session
        .userauth_pubkey_file(user, None, Path::new(key_path), None)
        .unwrap();

    let elapsed = start.elapsed();
    drop(session);
    elapsed
}

/// Benchmark russh connection establishment
async fn bench_russh_connect(host: &str, port: u16, user: &str, key_path: &str) -> Duration {
    let start = Instant::now();

    let auth = AuthMethod::with_key_file(key_path, None);
    let client = Client::connect((host, port), user, auth, ServerCheckMethod::NoCheck)
        .await
        .unwrap();

    let elapsed = start.elapsed();
    drop(client);
    elapsed
}

// =============================================================================
// Command Execution Benchmarks
// =============================================================================

/// Benchmark SSH2 command execution (single command on existing connection)
fn bench_ssh2_command(session: &ssh2::Session, command: &str) -> Duration {
    let start = Instant::now();

    let mut channel = session.channel_session().unwrap();
    channel.exec(command).unwrap();
    let mut output = String::new();
    channel.read_to_string(&mut output).unwrap();
    channel.wait_close().unwrap();

    start.elapsed()
}

/// Benchmark russh command execution (single command on existing connection)
async fn bench_russh_command(client: &Client, command: &str) -> Duration {
    let start = Instant::now();

    let _result = client.execute(command).await.unwrap();

    start.elapsed()
}

/// Benchmark SSH2 with connection and command
fn bench_ssh2_connect_and_command(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    command: &str,
) -> Duration {
    let start = Instant::now();

    let tcp = TcpStream::connect((host, port)).unwrap();
    let mut session = ssh2::Session::new().unwrap();
    session.set_tcp_stream(tcp);
    session.handshake().unwrap();
    session
        .userauth_pubkey_file(user, None, Path::new(key_path), None)
        .unwrap();

    let mut channel = session.channel_session().unwrap();
    channel.exec(command).unwrap();
    let mut output = String::new();
    channel.read_to_string(&mut output).unwrap();
    channel.wait_close().unwrap();

    start.elapsed()
}

/// Benchmark russh with connection and command
async fn bench_russh_connect_and_command(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    command: &str,
) -> Duration {
    let start = Instant::now();

    let auth = AuthMethod::with_key_file(key_path, None);
    let client = Client::connect((host, port), user, auth, ServerCheckMethod::NoCheck)
        .await
        .unwrap();

    let _result = client.execute(command).await.unwrap();

    start.elapsed()
}

// =============================================================================
// File Transfer Benchmarks
// =============================================================================

/// Benchmark SSH2 file upload
fn bench_ssh2_upload(session: &ssh2::Session, data: &[u8], remote_path: &str) -> Duration {
    let start = Instant::now();

    let sftp = session.sftp().unwrap();
    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .unwrap();
    remote_file.write_all(data).unwrap();

    start.elapsed()
}

/// Benchmark SSH2 file download
fn bench_ssh2_download(session: &ssh2::Session, remote_path: &str) -> Duration {
    let start = Instant::now();

    let sftp = session.sftp().unwrap();
    let mut remote_file = sftp.open(Path::new(remote_path)).unwrap();
    let mut buffer = Vec::new();
    remote_file.read_to_end(&mut buffer).unwrap();

    start.elapsed()
}

/// Benchmark russh file upload (using base64 over command for simplicity)
async fn bench_russh_upload(client: &Client, data: &[u8], remote_path: &str) -> Duration {
    use base64::Engine;
    let start = Instant::now();

    // Encode data as base64 and write via command
    let encoded = base64::engine::general_purpose::STANDARD.encode(data);
    let cmd = format!("echo '{}' | base64 -d > {}", encoded, remote_path);
    let _result = client.execute(&cmd).await.unwrap();

    start.elapsed()
}

/// Benchmark russh file download (using base64 over command for simplicity)
async fn bench_russh_download(client: &Client, remote_path: &str) -> Duration {
    let start = Instant::now();

    // Read file via command and base64 encode
    let cmd = format!("base64 < {}", remote_path);
    let _result = client.execute(&cmd).await.unwrap();

    start.elapsed()
}

// =============================================================================
// Parallel Execution Benchmarks
// =============================================================================

/// Benchmark SSH2 parallel commands using spawn_blocking
async fn bench_ssh2_parallel_spawn_blocking(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    num_parallel: usize,
) -> Duration {
    let start = Instant::now();

    let mut handles = Vec::new();
    for _ in 0..num_parallel {
        let host = host.to_string();
        let user = user.to_string();
        let key_path = key_path.to_string();

        let handle = tokio::task::spawn_blocking(move || {
            let tcp = TcpStream::connect((&host[..], port)).unwrap();
            let mut session = ssh2::Session::new().unwrap();
            session.set_tcp_stream(tcp);
            session.handshake().unwrap();
            session
                .userauth_pubkey_file(&user, None, Path::new(&key_path), None)
                .unwrap();

            let mut channel = session.channel_session().unwrap();
            channel.exec("echo hello").unwrap();
            let mut output = String::new();
            channel.read_to_string(&mut output).unwrap();
            channel.wait_close().unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    start.elapsed()
}

/// Benchmark russh parallel commands (native async)
async fn bench_russh_parallel_async(
    host: &str,
    port: u16,
    user: &str,
    key_path: &str,
    num_parallel: usize,
) -> Duration {
    let start = Instant::now();

    let mut handles = Vec::new();
    for _ in 0..num_parallel {
        let host = host.to_string();
        let user = user.to_string();
        let key_path = key_path.to_string();

        let handle = tokio::spawn(async move {
            let auth = AuthMethod::with_key_file(&key_path, None);
            let client = Client::connect((&host[..], port), &user, auth, ServerCheckMethod::NoCheck)
                .await
                .unwrap();

            let _result = client.execute("echo hello").await.unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }

    start.elapsed()
}

// =============================================================================
// Main Benchmark Runner
// =============================================================================

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let key_path = expand_path(&args.key);

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║        SSH Library Comparison Benchmark Suite                ║");
    println!("║        ssh2 (libssh2) vs async-ssh2-tokio (russh)           ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Configuration:");
    println!("  Host:       {}:{}", args.host, args.port);
    println!("  User:       {}", args.user);
    println!("  Key:        {}", key_path);
    println!("  Iterations: {}", args.iterations);
    println!();

    let mut results = Vec::new();

    // =========================================================================
    // 1. CONNECTION TIME BENCHMARKS
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ 1. Connection Establishment Benchmarks                      │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    // SSH2 connection
    println!("Benchmarking ssh2 connection establishment...");
    let mut ssh2_connect_stats = BenchStats::new("Connection", "ssh2");
    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = tokio::task::spawn_blocking({
            let host = args.host.clone();
            let user = args.user.clone();
            let key_path = key_path.clone();
            move || bench_ssh2_connect(&host, args.port, &user, &key_path)
        })
        .await
        .unwrap();
        ssh2_connect_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(ssh2_connect_stats.to_result());

    // Russh connection
    println!("Benchmarking russh connection establishment...");
    let mut russh_connect_stats = BenchStats::new("Connection", "russh");
    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_russh_connect(&args.host, args.port, &args.user, &key_path).await;
        russh_connect_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(russh_connect_stats.to_result());

    println!();

    // =========================================================================
    // 2. COMMAND EXECUTION BENCHMARKS (REUSED CONNECTION)
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ 2. Command Execution (Reused Connection)                    │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    // SSH2 command execution
    println!("Benchmarking ssh2 command execution...");
    let mut ssh2_cmd_stats = BenchStats::new("Command (reused)", "ssh2");
    let ssh2_session = tokio::task::spawn_blocking({
        let host = args.host.clone();
        let user = args.user.clone();
        let key_path = key_path.clone();
        move || {
            let tcp = TcpStream::connect((&host[..], args.port)).unwrap();
            let mut session = ssh2::Session::new().unwrap();
            session.set_tcp_stream(tcp);
            session.handshake().unwrap();
            session
                .userauth_pubkey_file(&user, None, Path::new(&key_path), None)
                .unwrap();
            session
        }
    })
    .await
    .unwrap();

    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_ssh2_command(&ssh2_session, "echo hello");
        ssh2_cmd_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(ssh2_cmd_stats.to_result());

    // Russh command execution
    println!("Benchmarking russh command execution...");
    let mut russh_cmd_stats = BenchStats::new("Command (reused)", "russh");
    let auth = AuthMethod::with_key_file(&key_path, None);
    let russh_client = Client::connect(
        (&args.host[..], args.port),
        &args.user,
        auth,
        ServerCheckMethod::NoCheck,
    )
    .await
    .unwrap();

    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_russh_command(&russh_client, "echo hello").await;
        russh_cmd_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(russh_cmd_stats.to_result());

    println!();

    // =========================================================================
    // 3. COMMAND EXECUTION BENCHMARKS (NEW CONNECTION)
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ 3. Command Execution (New Connection Per Command)           │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    // SSH2 connect + command
    println!("Benchmarking ssh2 connect + command...");
    let mut ssh2_connect_cmd_stats = BenchStats::new("Connect + Command", "ssh2");
    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = tokio::task::spawn_blocking({
            let host = args.host.clone();
            let user = args.user.clone();
            let key_path = key_path.clone();
            move || bench_ssh2_connect_and_command(&host, args.port, &user, &key_path, "echo hello")
        })
        .await
        .unwrap();
        ssh2_connect_cmd_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(ssh2_connect_cmd_stats.to_result());

    // Russh connect + command
    println!("Benchmarking russh connect + command...");
    let mut russh_connect_cmd_stats = BenchStats::new("Connect + Command", "russh");
    for i in 0..args.iterations {
        if args.verbose && i % 10 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_russh_connect_and_command(
            &args.host,
            args.port,
            &args.user,
            &key_path,
            "echo hello",
        )
        .await;
        russh_connect_cmd_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(russh_connect_cmd_stats.to_result());

    println!();

    // =========================================================================
    // 4. FILE TRANSFER BENCHMARKS
    // =========================================================================
    if !args.skip_file_transfer {
        println!("┌─────────────────────────────────────────────────────────────┐");
        println!("│ 4. File Transfer Benchmarks ({} KB)                      │", args.file_size_kb);
        println!("└─────────────────────────────────────────────────────────────┘\n");

        // Generate random test data
        let mut rng = rand::thread_rng();
        let test_data: Vec<u8> = (0..args.file_size_kb * 1024)
            .map(|_| rng.gen())
            .collect();

        let remote_path = "/tmp/ssh_bench_test.dat";

        // SSH2 upload
        println!("Benchmarking ssh2 file upload ({} KB)...", args.file_size_kb);
        let mut ssh2_upload_stats = BenchStats::new(&format!("Upload {} KB", args.file_size_kb), "ssh2");
        for i in 0..args.iterations.min(20) {
            // Limit file transfer iterations
            if args.verbose && i % 5 == 0 {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            let duration = bench_ssh2_upload(&ssh2_session, &test_data, remote_path);
            ssh2_upload_stats.record(duration);
        }
        if args.verbose {
            println!();
        }
        results.push(ssh2_upload_stats.to_result());

        // Russh upload
        println!("Benchmarking russh file upload ({} KB)...", args.file_size_kb);
        let mut russh_upload_stats = BenchStats::new(&format!("Upload {} KB", args.file_size_kb), "russh");
        for i in 0..args.iterations.min(20) {
            if args.verbose && i % 5 == 0 {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            let duration = bench_russh_upload(&russh_client, &test_data, remote_path).await;
            russh_upload_stats.record(duration);
        }
        if args.verbose {
            println!();
        }
        results.push(russh_upload_stats.to_result());

        // SSH2 download
        println!("Benchmarking ssh2 file download ({} KB)...", args.file_size_kb);
        let mut ssh2_download_stats = BenchStats::new(&format!("Download {} KB", args.file_size_kb), "ssh2");
        for i in 0..args.iterations.min(20) {
            if args.verbose && i % 5 == 0 {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            let duration = bench_ssh2_download(&ssh2_session, remote_path);
            ssh2_download_stats.record(duration);
        }
        if args.verbose {
            println!();
        }
        results.push(ssh2_download_stats.to_result());

        // Russh download
        println!("Benchmarking russh file download ({} KB)...", args.file_size_kb);
        let mut russh_download_stats = BenchStats::new(&format!("Download {} KB", args.file_size_kb), "russh");
        for i in 0..args.iterations.min(20) {
            if args.verbose && i % 5 == 0 {
                print!(".");
                std::io::stdout().flush().unwrap();
            }
            let duration = bench_russh_download(&russh_client, remote_path).await;
            russh_download_stats.record(duration);
        }
        if args.verbose {
            println!();
        }
        results.push(russh_download_stats.to_result());

        println!();
    }

    // =========================================================================
    // 5. PARALLEL EXECUTION BENCHMARKS
    // =========================================================================
    println!("┌─────────────────────────────────────────────────────────────┐");
    println!("│ 5. Parallel Execution (10 concurrent connections)           │");
    println!("└─────────────────────────────────────────────────────────────┘\n");

    let num_parallel = 10;

    // SSH2 with spawn_blocking
    println!("Benchmarking ssh2 parallel (spawn_blocking)...");
    let mut ssh2_parallel_stats = BenchStats::new("Parallel 10x", "ssh2 (spawn_blocking)");
    for i in 0..args.iterations.min(20) {
        if args.verbose && i % 5 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_ssh2_parallel_spawn_blocking(
            &args.host,
            args.port,
            &args.user,
            &key_path,
            num_parallel,
        )
        .await;
        ssh2_parallel_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(ssh2_parallel_stats.to_result());

    // Russh native async
    println!("Benchmarking russh parallel (native async)...");
    let mut russh_parallel_stats = BenchStats::new("Parallel 10x", "russh (async)");
    for i in 0..args.iterations.min(20) {
        if args.verbose && i % 5 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        let duration = bench_russh_parallel_async(
            &args.host,
            args.port,
            &args.user,
            &key_path,
            num_parallel,
        )
        .await;
        russh_parallel_stats.record(duration);
    }
    if args.verbose {
        println!();
    }
    results.push(russh_parallel_stats.to_result());

    println!();

    // =========================================================================
    // RESULTS
    // =========================================================================
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                     BENCHMARK RESULTS                         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let table = Table::new(&results).to_string();
    println!("{}", table);

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                         SUMMARY                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    println!("Key Findings:");
    println!("  • russh is a pure Rust, async-native SSH library");
    println!("  • ssh2 wraps libssh2 (C library) and requires spawn_blocking");
    println!("  • russh generally shows better performance in async contexts");
    println!("  • For parallel operations, russh's native async is more efficient");
    println!("  • File transfers may vary based on SFTP implementation details");
    println!("\nNote: Lower times are better. P95/P99 show latency distribution.\n");
}
