use criterion::{Criterion, black_box, criterion_group, criterion_main};
use dashmap::DashMap;
use push_cache::model::Customer;
use rand::Rng;
use rand::seq::SliceRandom;
use std::sync::Arc;

fn create_customer(id: &str) -> Customer {
    Customer {
        accountId: id.to_string(),
        name: "Test User".to_string(),
        address: "123 Test Lane".to_string(),
        phone: "555-0123".to_string(),
        createdAt: 1000,
        updatedAt: 2000,
    }
}

fn benchmark_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    // Setup initial data
    let cache = Arc::new(DashMap::new());
    let mut keys = Vec::new();
    for i in 0..10_000 {
        let key = format!("user_{}", i);
        cache.insert(key.clone(), create_customer(&key));
        keys.push(key);
    }

    // 1. Read Heavy (95% Get, 5% Insert)
    group.bench_function("read_heavy_95_5", |b| {
        b.iter(|| {
            let mut rng = rand::thread_rng();
            let key = keys.choose(&mut rng).unwrap();

            if rng.gen_bool(0.05) {
                // Write
                cache.insert(key.clone(), create_customer(key));
            } else {
                // Read
                black_box(cache.get(key));
            }
        });
    });

    // 2. Write Heavy (50% Get, 50% Insert)
    group.bench_function("write_heavy_50_50", |b| {
        b.iter(|| {
            let mut rng = rand::thread_rng();
            let key = keys.choose(&mut rng).unwrap();

            if rng.gen_bool(0.5) {
                // Write
                cache.insert(key.clone(), create_customer(key));
            } else {
                // Read
                black_box(cache.get(key));
            }
        });
    });

    // 3. Bursty (High contention on single key)
    group.bench_function("bursty_single_key", |b| {
        let hot_key = &keys[0];
        b.iter(|| {
            let mut rng = rand::thread_rng();
            if rng.gen_bool(0.1) {
                cache.insert(hot_key.clone(), create_customer(hot_key));
            } else {
                black_box(cache.get(hot_key));
            }
        });
    });

    group.finish();
}

fn benchmark_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_growth");
    // Define the sizes we want to test
    let sizes = [
        100, 1_000, 10_000, 100_000, 1_000_000, 5_000_000, 10_000_000, 16_000_000,
    ];

    for size in sizes.iter() {
        group.bench_with_input(
            criterion::BenchmarkId::from_parameter(size),
            size,
            |b, &s| {
                // Setup DashMap with s entries
                let cache = Arc::new(DashMap::new());
                let mut keys = Vec::with_capacity(s);
                for i in 0..s {
                    let key = format!("user_{}", i);
                    cache.insert(key.clone(), create_customer(&key));
                    keys.push(key);
                }

                // Bench get on a random key
                b.iter(|| {
                    let mut rng = rand::thread_rng();
                    let key = keys.choose(&mut rng).unwrap();
                    black_box(cache.get(key));
                })
            },
        );
    }
    group.finish();
}

fn benchmark_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_performance");

    // Benchmark raw insert speed (overwrite)
    group.bench_function("insert_overwrite", |b| {
        let cache = DashMap::new();
        let customer = create_customer("temp");
        b.iter(|| {
            // Overwriting the same key to measure map overhead without memory growth
            cache.insert("key".to_string(), customer.clone());
        })
    });

    group.finish();
}

criterion_group!(benches, benchmark_cache, benchmark_growth, benchmark_insert);
criterion_main!(benches);
