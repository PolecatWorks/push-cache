use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use dashmap::DashMap;
use push_cache::model::Customer;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::sync::Arc;
use tokio::runtime::Runtime;

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

fn benchmark_concurrent(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // Fixed concurrency high load
    let concurrency = 100;

    // Varying sizes
    let sizes = [100_000, 1_000_000, 5_000_000];

    let mut group = c.benchmark_group("concurrent_size_impact");
    group.throughput(Throughput::Elements(concurrency as u64));

    for &size in sizes.iter() {
        group.bench_with_input(BenchmarkId::new("concurrent_100", size), &size, |b, &s| {
            // Setup data for this size
            let cache = Arc::new(DashMap::new());
            let mut keys = Vec::with_capacity(s);
            for i in 0..s {
                let key = format!("user_{}", i);
                cache.insert(key.clone(), create_customer(&key));
                keys.push(key);
            }
            let keys = Arc::new(keys);

            b.to_async(&rt).iter(|| async {
                let mut handles = Vec::with_capacity(concurrency);
                for _ in 0..concurrency {
                    let c = cache.clone();
                    let k = keys.clone();
                    handles.push(tokio::spawn(async move {
                        let mut rng = thread_rng();
                        let key = k.choose(&mut rng).unwrap();
                        c.get(key);
                    }));
                }

                for handle in handles {
                    handle.await.unwrap();
                }
            })
        });
    }
    group.finish();
}

criterion_group!(benches, benchmark_concurrent);
criterion_main!(benches);
