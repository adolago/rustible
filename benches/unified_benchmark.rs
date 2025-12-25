//! Unified Benchmark Suite for Rustible
//!
//! This benchmark provides a comprehensive overview of Rustible performance across
//! all major subsystems. It consolidates key benchmarks from specialized suites
//! for quick regression testing.
//!
//! ## Benchmark Categories:
//!
//! 1. **Playbook Parsing** - YAML parsing at various scales
//! 2. **Inventory Parsing** - Host/group loading (10/100/1000 hosts)
//! 3. **Module Execution** - Module dispatch and execution overhead
//! 4. **Connection Pool** - Connection establishment and pooling
//! 5. **Template Rendering** - Jinja2-compatible template engine
//! 6. **Full Playbook Runs** - End-to-end execution simulation
//!
//! ## Usage:
//!
//! ```bash
//! # Run all unified benchmarks
//! cargo bench --bench unified_benchmark
//!
//! # Run specific category
//! cargo bench --bench unified_benchmark -- playbook
//! cargo bench --bench unified_benchmark -- inventory
//! cargo bench --bench unified_benchmark -- module
//! cargo bench --bench unified_benchmark -- connection
//! cargo bench --bench unified_benchmark -- template
//! cargo bench --bench unified_benchmark -- full_run
//!
//! # Compare against baseline
//! cargo bench --bench unified_benchmark -- --baseline baseline
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;

use rustible::connection::{ConnectionConfig, ConnectionFactory};
use rustible::executor::playbook::Playbook;
use rustible::executor::task::Task;
use rustible::inventory::Inventory;
use rustible::modules::{ModuleContext, ModuleOutput, ModuleParams, ModuleRegistry};
use rustible::template::TemplateEngine;

// ============================================================================
// DATA GENERATORS
// ============================================================================

/// Generate playbook YAML with specified number of tasks
fn generate_playbook_yaml(num_tasks: usize) -> String {
    let mut yaml = String::from(
        r#"
- name: Unified Benchmark Play
  hosts: all
  gather_facts: false
  vars:
    app_name: benchmark
    version: "1.0.0"
  tasks:
"#,
    );

    for i in 0..num_tasks {
        yaml.push_str(&format!(
            r#"    - name: Task {}
      debug:
        msg: "Executing task {}"
"#,
            i, i
        ));
    }

    yaml
}

/// Generate inventory YAML with specified number of hosts
fn generate_inventory_yaml(num_hosts: usize) -> String {
    let num_groups = (num_hosts / 50).max(1);
    let hosts_per_group = (num_hosts / num_groups).max(1);
    let mut yaml = String::from("all:\n  children:\n");

    for g in 0..num_groups {
        yaml.push_str(&format!("    group_{:04}:\n      hosts:\n", g));
        let start = g * hosts_per_group;
        let end = ((g + 1) * hosts_per_group).min(num_hosts);
        for h in start..end {
            yaml.push_str(&format!(
                "        host{:05}:\n          ansible_host: 10.{}.{}.{}\n",
                h,
                (h / 65536) % 256,
                (h / 256) % 256,
                h % 256,
            ));
        }
    }

    yaml.push_str("  vars:\n    env: production\n");
    yaml
}

// ============================================================================
// PLAYBOOK PARSING BENCHMARKS
// ============================================================================

fn bench_playbook_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("playbook_parsing");

    // Small playbook (5 tasks)
    let small_yaml = generate_playbook_yaml(5);
    group.throughput(Throughput::Elements(5));
    group.bench_function("small_5_tasks", |b| {
        b.iter(|| {
            let result = Playbook::parse(black_box(&small_yaml), None);
            black_box(result)
        })
    });

    // Medium playbook (20 tasks)
    let medium_yaml = generate_playbook_yaml(20);
    group.throughput(Throughput::Elements(20));
    group.bench_function("medium_20_tasks", |b| {
        b.iter(|| {
            let result = Playbook::parse(black_box(&medium_yaml), None);
            black_box(result)
        })
    });

    // Large playbook (100 tasks)
    let large_yaml = generate_playbook_yaml(100);
    group.throughput(Throughput::Elements(100));
    group.bench_function("large_100_tasks", |b| {
        b.iter(|| {
            let result = Playbook::parse(black_box(&large_yaml), None);
            black_box(result)
        })
    });

    group.finish();
}

// ============================================================================
// INVENTORY PARSING BENCHMARKS
// ============================================================================

fn bench_inventory_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("inventory_parsing");

    for num_hosts in [10, 100, 1000] {
        let yaml = generate_inventory_yaml(num_hosts);

        group.throughput(Throughput::Elements(num_hosts as u64));
        group.bench_with_input(
            BenchmarkId::new("hosts", num_hosts),
            &yaml,
            |b, yaml_content| {
                b.iter(|| {
                    let mut tmpfile = NamedTempFile::new().unwrap();
                    tmpfile.write_all(yaml_content.as_bytes()).unwrap();
                    tmpfile.flush().unwrap();
                    let result = Inventory::load(black_box(tmpfile.path()));
                    black_box(result)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// MODULE EXECUTION BENCHMARKS
// ============================================================================

fn bench_module_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("module_execution");

    let registry = ModuleRegistry::with_builtins();
    let context = ModuleContext::new().with_check_mode(true);

    // Module lookup
    group.bench_function("lookup_exists", |b| {
        b.iter(|| {
            let module = registry.get(black_box("debug"));
            black_box(module)
        })
    });

    group.bench_function("lookup_missing", |b| {
        b.iter(|| {
            let module = registry.get(black_box("nonexistent"));
            black_box(module)
        })
    });

    // Module output creation
    group.bench_function("output_ok", |b| {
        b.iter(|| {
            let output = ModuleOutput::ok(black_box("Success"));
            black_box(output)
        })
    });

    group.bench_function("output_changed_with_data", |b| {
        b.iter(|| {
            let output = ModuleOutput::changed(black_box("File modified"))
                .with_data("path", serde_json::json!("/etc/config.conf"))
                .with_data("mode", serde_json::json!("0644"))
                .with_data("owner", serde_json::json!("root"));
            black_box(output)
        })
    });

    // Parameter creation
    group.bench_function("params_creation", |b| {
        b.iter(|| {
            let mut params: ModuleParams = HashMap::new();
            params.insert("msg".to_string(), serde_json::json!("Hello, World!"));
            params.insert("verbosity".to_string(), serde_json::json!(0));
            black_box(params)
        })
    });

    // Module execution (debug module in check mode)
    let debug_module = registry.get("debug").unwrap();
    let mut params: ModuleParams = HashMap::new();
    params.insert("msg".to_string(), serde_json::json!("Benchmark message"));

    group.bench_function("execute_debug", |b| {
        b.iter(|| {
            let result = debug_module.execute(black_box(&params), black_box(&context));
            black_box(result)
        })
    });

    group.finish();
}

// ============================================================================
// CONNECTION BENCHMARKS
// ============================================================================

fn bench_connection_pool(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("connection_pool");

    // Factory creation
    group.bench_function("factory_create", |b| {
        b.iter(|| {
            let config = ConnectionConfig::default();
            let factory = ConnectionFactory::new(black_box(config));
            black_box(factory)
        })
    });

    // Local connection (fastest path)
    let factory = Arc::new(ConnectionFactory::new(ConnectionConfig::default()));
    group.bench_function("local_connection", |b| {
        b.to_async(&rt).iter(|| {
            let factory = Arc::clone(&factory);
            async move {
                let conn = factory.get_connection(black_box("localhost")).await;
                black_box(conn)
            }
        })
    });

    // Pool stats
    group.bench_function("pool_stats", |b| {
        let factory = ConnectionFactory::new(ConnectionConfig::default());
        b.iter(|| {
            let stats = factory.pool_stats();
            black_box(stats)
        })
    });

    group.finish();
}

// ============================================================================
// TEMPLATE BENCHMARKS
// ============================================================================

fn bench_template_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_rendering");

    let engine = TemplateEngine::new();

    // Simple template
    let simple_template = "Hello {{ name }}!";
    let mut simple_vars = HashMap::new();
    simple_vars.insert("name".to_string(), serde_json::json!("World"));

    group.bench_function("simple", |b| {
        b.iter(|| {
            let result = engine.render(black_box(simple_template), black_box(&simple_vars));
            black_box(result)
        })
    });

    // Medium template
    let medium_template =
        "Server: {{ server }}, Port: {{ port }}, User: {{ user }}, Env: {{ env }}";
    let mut medium_vars = HashMap::new();
    medium_vars.insert("server".to_string(), serde_json::json!("localhost"));
    medium_vars.insert("port".to_string(), serde_json::json!(8080));
    medium_vars.insert("user".to_string(), serde_json::json!("admin"));
    medium_vars.insert("env".to_string(), serde_json::json!("production"));

    group.bench_function("medium", |b| {
        b.iter(|| {
            let result = engine.render(black_box(medium_template), black_box(&medium_vars));
            black_box(result)
        })
    });

    // Template detection
    group.bench_function("is_template_true", |b| {
        b.iter(|| TemplateEngine::is_template(black_box("Hello {{ name }}")))
    });

    group.bench_function("is_template_false", |b| {
        b.iter(|| TemplateEngine::is_template(black_box("Hello World")))
    });

    group.finish();
}

// ============================================================================
// FULL PLAYBOOK RUN SIMULATION
// ============================================================================

fn bench_full_playbook_run(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("full_playbook_run");
    group.sample_size(30);

    // Simulate parallel execution across hosts
    for (num_hosts, num_tasks) in [(10, 5), (50, 10), (100, 20)] {
        group.throughput(Throughput::Elements((num_hosts * num_tasks) as u64));

        group.bench_with_input(
            BenchmarkId::new("hosts_tasks", format!("{}h_{}t", num_hosts, num_tasks)),
            &(num_hosts, num_tasks),
            |b, &(hosts, tasks)| {
                b.to_async(&rt).iter(|| async move {
                    let semaphore = Arc::new(Semaphore::new(5)); // 5 forks
                    let mut handles = Vec::new();

                    // Simulate task execution across hosts
                    for task_id in 0..tasks {
                        for host_id in 0..hosts {
                            let sem = Arc::clone(&semaphore);
                            handles.push(tokio::spawn(async move {
                                let _permit = sem.acquire().await.unwrap();
                                // Simulate minimal task work
                                tokio::task::yield_now().await;
                                (host_id, task_id, "ok")
                            }));
                        }
                    }

                    let mut results = Vec::with_capacity(handles.len());
                    for handle in handles {
                        results.push(handle.await.unwrap());
                    }
                    black_box(results)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// TASK OPERATIONS
// ============================================================================

fn bench_task_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_operations");

    // Task creation
    group.bench_function("create_simple", |b| {
        b.iter(|| {
            let task = Task::new(black_box("Test Task"), black_box("debug"))
                .arg("msg", serde_json::json!("Hello"));
            black_box(task)
        })
    });

    group.bench_function("create_complex", |b| {
        b.iter(|| {
            let task = Task::new(black_box("Complex Task"), black_box("template"))
                .arg("src", serde_json::json!("template.j2"))
                .arg("dest", serde_json::json!("/etc/config.conf"))
                .arg("owner", serde_json::json!("root"))
                .arg("mode", serde_json::json!("0644"))
                .when("ansible_os_family == 'Debian'")
                .notify("restart service")
                .register("result");
            black_box(task)
        })
    });

    // Task cloning
    let simple_task = Task::new("Clone Test", "debug").arg("msg", serde_json::json!("Test"));

    let complex_task = Task::new("Complex Clone", "template")
        .arg("src", serde_json::json!("template.j2"))
        .arg("dest", serde_json::json!("/etc/config.conf"))
        .when("condition == true")
        .notify("handler");

    group.bench_function("clone_simple", |b| b.iter(|| black_box(simple_task.clone())));

    group.bench_function("clone_complex", |b| b.iter(|| black_box(complex_task.clone())));

    group.finish();
}

// ============================================================================
// CRITERION GROUPS AND MAIN
// ============================================================================

criterion_group!(
    playbook_benches,
    bench_playbook_parsing,
);

criterion_group!(
    inventory_benches,
    bench_inventory_parsing,
);

criterion_group!(
    module_benches,
    bench_module_execution,
);

criterion_group!(
    connection_benches,
    bench_connection_pool,
);

criterion_group!(
    template_benches,
    bench_template_rendering,
);

criterion_group!(
    full_run_benches,
    bench_full_playbook_run,
);

criterion_group!(
    task_benches,
    bench_task_operations,
);

criterion_main!(
    playbook_benches,
    inventory_benches,
    module_benches,
    connection_benches,
    template_benches,
    full_run_benches,
    task_benches,
);
