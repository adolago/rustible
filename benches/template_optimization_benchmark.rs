//! Template Optimization Benchmarks
//!
//! This benchmark suite measures the performance improvements from:
//! - Template precompilation cache
//! - Lazy variable evaluation
//! - Trie-based variable path resolution
//! - Profiling overhead

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::HashMap;
use serde_json::json;

// Import the optimized template engine
use rustible::template::{
    TemplateEngine, TemplateEngineConfig,
    TemplateCache, TemplateCacheConfig, CompiledTemplate,
    VariableTrie, PathResolver,
    LazyProviderBuilder,
    TemplateProfiler,
};

// ============================================================================
// Test Data Generators
// ============================================================================

fn generate_simple_vars() -> HashMap<String, serde_json::Value> {
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), json!("World"));
    vars.insert("count".to_string(), json!(42));
    vars
}

fn generate_nested_vars(depth: usize) -> HashMap<String, serde_json::Value> {
    let mut nested = json!("leaf_value");
    for i in (0..depth).rev() {
        nested = json!({ format!("level{}", i): nested });
    }

    let mut vars = HashMap::new();
    vars.insert("config".to_string(), nested);
    vars
}

fn generate_many_vars(count: usize) -> HashMap<String, serde_json::Value> {
    let mut vars = HashMap::new();
    for i in 0..count {
        vars.insert(format!("var{}", i), json!(format!("value{}", i)));
    }
    vars
}

fn generate_complex_nested() -> HashMap<String, serde_json::Value> {
    let mut vars = HashMap::new();
    vars.insert("config".to_string(), json!({
        "database": {
            "primary": {
                "host": "localhost",
                "port": 5432,
                "username": "admin"
            },
            "replica": {
                "host": "replica.local",
                "port": 5432
            }
        },
        "cache": {
            "redis": {
                "host": "redis.local",
                "port": 6379
            }
        },
        "api": {
            "endpoints": {
                "users": "/api/v1/users",
                "orders": "/api/v1/orders"
            }
        }
    }));
    vars
}

// ============================================================================
// Cache Benchmarks
// ============================================================================

fn bench_cache_vs_no_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("template_cache");

    let template = "Hello {{ name }}, you have {{ count }} messages!";
    let vars = generate_simple_vars();

    // Without cache
    let engine_no_cache = TemplateEngine::with_config(TemplateEngineConfig::baseline());

    group.bench_function("no_cache", |b| {
        b.iter(|| {
            let result = engine_no_cache.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    // With cache
    let engine_cached = TemplateEngine::with_config(TemplateEngineConfig::production());

    // Warm the cache
    let _ = engine_cached.render(template, &vars);

    group.bench_function("with_cache", |b| {
        b.iter(|| {
            let result = engine_cached.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    group.finish();
}

fn bench_cache_cold_vs_warm(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_cold_warm");

    let template = "Complex template: {{ a }} + {{ b }} = {{ c }}";
    let mut vars = HashMap::new();
    vars.insert("a".to_string(), json!(10));
    vars.insert("b".to_string(), json!(20));
    vars.insert("c".to_string(), json!(30));

    // Cold cache (first access)
    group.bench_function("cold_cache", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;
            for i in 0..iters {
                let engine = TemplateEngine::new();
                let template_i = format!("{} - {}", template, i);
                let start = std::time::Instant::now();
                let _ = engine.render(&template_i, &vars);
                total += start.elapsed();
            }
            total
        })
    });

    // Warm cache
    let engine = TemplateEngine::new();
    let _ = engine.render(template, &vars);

    group.bench_function("warm_cache", |b| {
        b.iter(|| {
            let result = engine.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    group.finish();
}

fn bench_cache_lru_eviction(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_lru");

    let config = TemplateCacheConfig {
        max_entries: 100,
        ttl_secs: 0,
    };
    let cache = TemplateCache::new(config);

    // Fill cache to capacity
    for i in 0..100 {
        let template = format!("template{}", i);
        cache.insert(&template, CompiledTemplate::new(template.clone()));
    }

    // Benchmark insertion with eviction
    group.bench_function("insert_with_eviction", |b| {
        let mut counter = 100;
        b.iter(|| {
            let template = format!("new_template{}", counter);
            cache.insert(black_box(&template), CompiledTemplate::new(template.clone()));
            counter += 1;
        })
    });

    // Benchmark lookup
    group.bench_function("lookup", |b| {
        b.iter(|| {
            let result = cache.get(black_box("template50"));
            black_box(result)
        })
    });

    group.finish();
}

// ============================================================================
// Trie Benchmarks
// ============================================================================

fn bench_trie_vs_naive_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_lookup");

    let vars = generate_complex_nested();

    // Build trie
    let trie = VariableTrie::from_json_map(&vars);

    // Naive lookup path
    fn naive_lookup<'a>(vars: &'a HashMap<String, serde_json::Value>, path: &str) -> Option<&'a serde_json::Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = vars.get(parts[0])?;
        for part in &parts[1..] {
            current = current.get(part)?;
        }
        Some(current)
    }

    let path = "config.database.primary.host";

    group.bench_function("naive_lookup", |b| {
        b.iter(|| {
            let result = naive_lookup(black_box(&vars), black_box(path));
            black_box(result)
        })
    });

    group.bench_function("trie_lookup", |b| {
        b.iter(|| {
            let result = trie.get_dotted(black_box(path));
            black_box(result)
        })
    });

    // With path resolver caching
    let mut resolver = PathResolver::new(&trie);
    // Warm the resolver cache
    let _ = resolver.resolve(path);

    group.bench_function("cached_resolver", |b| {
        b.iter(|| {
            let result = resolver.resolve(black_box(path));
            black_box(result)
        })
    });

    group.finish();
}

fn bench_trie_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_depth");

    for depth in [3, 5, 7, 10] {
        let vars = generate_nested_vars(depth);
        let trie = VariableTrie::from_json_map(&vars);

        // Build path to leaf
        let path: String = (0..depth).map(|i| format!("level{}", i)).collect::<Vec<_>>().join(".");
        let full_path = format!("config.{}", path);

        group.throughput(Throughput::Elements(depth as u64));
        group.bench_with_input(BenchmarkId::from_parameter(depth), &depth, |b, _| {
            b.iter(|| {
                let result = trie.get_dotted(black_box(&full_path));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_trie_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("trie_construction");

    for count in [10, 50, 100, 500] {
        let vars = generate_many_vars(count);

        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let trie = VariableTrie::from_json_map(black_box(&vars));
                black_box(trie)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Lazy Evaluation Benchmarks
// ============================================================================

fn bench_lazy_vs_eager(c: &mut Criterion) {
    let mut group = c.benchmark_group("lazy_evaluation");

    // Simulate expensive variable computation
    fn expensive_computation() -> serde_json::Value {
        // Simulate some work
        let mut sum = 0u64;
        for i in 0..1000 {
            sum = sum.wrapping_add(i);
        }
        json!(sum)
    }

    // Eager: compute all variables upfront
    group.bench_function("eager_all_vars", |b| {
        b.iter(|| {
            let mut vars = HashMap::new();
            for i in 0..10 {
                vars.insert(format!("var{}", i), expensive_computation());
            }
            black_box(vars)
        })
    });

    // Lazy: only compute when accessed
    group.bench_function("lazy_one_var_accessed", |b| {
        b.iter(|| {
            let provider = LazyProviderBuilder::new()
                .lazy("var0", || expensive_computation())
                .lazy("var1", || expensive_computation())
                .lazy("var2", || expensive_computation())
                .lazy("var3", || expensive_computation())
                .lazy("var4", || expensive_computation())
                .lazy("var5", || expensive_computation())
                .lazy("var6", || expensive_computation())
                .lazy("var7", || expensive_computation())
                .lazy("var8", || expensive_computation())
                .lazy("var9", || expensive_computation())
                .build();

            // Only access one variable
            let val = provider.get("var0").map(|v| v.get());
            black_box(val)
        })
    });

    group.finish();
}

// ============================================================================
// Profiling Overhead Benchmarks
// ============================================================================

fn bench_profiling_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("profiling_overhead");

    let template = "Hello {{ name }}!";
    let vars = generate_simple_vars();

    // Without profiling
    let engine_no_profiling = TemplateEngine::with_config(TemplateEngineConfig {
        enable_profiling: false,
        ..TemplateEngineConfig::default()
    });

    group.bench_function("no_profiling", |b| {
        b.iter(|| {
            let result = engine_no_profiling.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    // With profiling
    let engine_profiling = TemplateEngine::with_config(TemplateEngineConfig {
        enable_profiling: true,
        ..TemplateEngineConfig::default()
    });

    group.bench_function("with_profiling", |b| {
        b.iter(|| {
            let result = engine_profiling.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    group.finish();
}

fn bench_profiler_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("profiler_ops");

    let profiler = TemplateProfiler::new();

    // Benchmark recording
    group.bench_function("record_render", |b| {
        b.iter(|| {
            profiler.record_render(
                black_box("{{ x }}"),
                black_box(std::time::Duration::from_micros(100)),
                black_box(5),
            );
        })
    });

    // Warm up with data
    for _ in 0..1000 {
        profiler.record_render("template", std::time::Duration::from_micros(50), 3);
    }

    // Benchmark stats retrieval
    group.bench_function("get_stats", |b| {
        b.iter(|| {
            let stats = profiler.get_stats();
            black_box(stats)
        })
    });

    group.bench_function("get_slowest", |b| {
        b.iter(|| {
            let slowest = profiler.get_slowest(10);
            black_box(slowest)
        })
    });

    group.finish();
}

// ============================================================================
// End-to-End Optimization Benchmarks
// ============================================================================

fn bench_full_optimization_stack(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_optimization");

    let template = r#"
Database Configuration:
  Host: {{ config.database.primary.host }}
  Port: {{ config.database.primary.port }}

Cache Configuration:
  Host: {{ config.cache.redis.host }}
  Port: {{ config.cache.redis.port }}

API Endpoints:
  Users: {{ config.api.endpoints.users }}
  Orders: {{ config.api.endpoints.orders }}
"#;

    let vars = generate_complex_nested();

    // Baseline (no optimizations)
    let engine_baseline = TemplateEngine::with_config(TemplateEngineConfig::baseline());

    group.bench_function("baseline", |b| {
        b.iter(|| {
            let result = engine_baseline.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    // Production (all optimizations)
    let engine_optimized = TemplateEngine::with_config(TemplateEngineConfig::production());
    // Warm the cache
    let _ = engine_optimized.render(template, &vars);

    group.bench_function("optimized", |b| {
        b.iter(|| {
            let result = engine_optimized.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    // With trie
    group.bench_function("with_trie", |b| {
        b.iter(|| {
            let result = engine_optimized.render_with_trie(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    group.finish();
}

fn bench_repeated_renders(c: &mut Criterion) {
    let mut group = c.benchmark_group("repeated_renders");

    let template = "{{ name }} - {{ value }}";
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), json!("test"));
    vars.insert("value".to_string(), json!(123));

    let engine = TemplateEngine::new();

    // Single render
    group.bench_function("single", |b| {
        b.iter(|| {
            let result = engine.render(black_box(template), black_box(&vars));
            black_box(result)
        })
    });

    // 10 repeated renders (simulates loop)
    group.bench_function("10_repeated", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let result = engine.render(template, &vars);
                black_box(result);
            }
        })
    });

    // 100 repeated renders
    group.bench_function("100_repeated", |b| {
        b.iter(|| {
            for _ in 0..100 {
                let result = engine.render(template, &vars);
                black_box(result);
            }
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    cache_benches,
    bench_cache_vs_no_cache,
    bench_cache_cold_vs_warm,
    bench_cache_lru_eviction,
);

criterion_group!(
    trie_benches,
    bench_trie_vs_naive_lookup,
    bench_trie_depth,
    bench_trie_construction,
);

criterion_group!(
    lazy_benches,
    bench_lazy_vs_eager,
);

criterion_group!(
    profiling_benches,
    bench_profiling_overhead,
    bench_profiler_operations,
);

criterion_group!(
    e2e_benches,
    bench_full_optimization_stack,
    bench_repeated_renders,
);

criterion_main!(
    cache_benches,
    trie_benches,
    lazy_benches,
    profiling_benches,
    e2e_benches,
);
