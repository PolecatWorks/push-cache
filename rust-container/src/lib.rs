use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{Arc, atomic::AtomicBool},
};

use apache_avro::Schema;
use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use dashmap::DashMap;
use tracing::{error, info};

use ::hams::hams::Hams;
use ::hams::probe::AsyncHealthProbe;
use ::hams::probe::FFIProbe;
use ::hams::probe::manual::Manual as ProbeManual;
use prometheus::{IntCounter, IntGauge, Registry};

use tokio_util::sync::CancellationToken;

use crate::{
    config::MyConfig, error::MyError, tokio_tools::run_in_tokio, webserver::start_app_api,
};

use metrics::{prometheus_response_free, prometheus_response_mystate};

use crate::cache::{Cache, InMemoryCache, MongoCache, OracleCache, PostgresCache, RedisCache};
use crate::startup_tools::run_startup_checks;

pub mod cache;
pub mod config;
pub mod consumer;
pub mod error;
pub mod hams;
pub mod kafka_utils;
mod metrics;
mod startup_tools;
pub mod tokio_tools;
pub mod webserver;

/// Name of the Crate
pub const NAME: &str = env!("CARGO_PKG_NAME");
/// Version of the Crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct MyState {
    config: MyConfig,
    pub stores: Arc<HashMap<String, Arc<dyn Cache + Send + Sync>>>,
    pub routes: Vec<crate::config::RouteDefinition>,
    pub schema_to_store: Arc<HashMap<String, String>>,
    pub schema_cache: Arc<DashMap<u32, Schema>>,
    // Metrics
    pub requests_total: Box<IntCounter>,
    pub requests_miss: Box<IntCounter>,
    pub updates_received: Box<IntCounter>,
    pub tombstones_processed: Box<IntCounter>,
    pub schema_mismatch_count: Box<IntCounter>,
    pub schema_unrouted_count: Box<IntCounter>,
    pub cache_size: Box<IntGauge>,
    pub consumer_lag: Box<IntGauge>,
    pub startup_lag_cleared: Arc<AtomicBool>,

    registry: Registry,
    prometheus_handle: Arc<PrometheusHandle>,
}

impl MyState {
    pub async fn new(config: &MyConfig) -> Result<MyState, MyError> {
        let registry = Registry::new();
        let perform_checks = config.startup_checks.enabled;

        let requests_total = IntCounter::new("requests_total", "Total user info requests")?;
        let requests_miss =
            IntCounter::new("requests_miss", "Total requests with no record found")?;
        let updates_received =
            IntCounter::new("updates_received", "Total updates received from Kafka")?;
        let tombstones_processed =
            IntCounter::new("tombstones_processed", "Total tombstone records processed")?;
        let schema_mismatch_count = IntCounter::new(
            "schema_mismatch_count",
            "Total messages with schema mismatch",
        )?;
        let schema_unrouted_count = IntCounter::new(
            "schema_unrouted_count",
            "Total messages where schema was not routed to any store",
        )?;
        let cache_size = IntGauge::new("push_cache_records_total", "Total records in cache")?;
        let consumer_lag =
            IntGauge::new("push_cache_consumer_lag_total", "Total Kafka consumer lag")?;

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(requests_miss.clone()))?;
        registry.register(Box::new(updates_received.clone()))?;
        registry.register(Box::new(tombstones_processed.clone()))?;
        registry.register(Box::new(schema_mismatch_count.clone()))?;
        registry.register(Box::new(schema_unrouted_count.clone()))?;
        registry.register(Box::new(cache_size.clone()))?;
        registry.register(Box::new(consumer_lag.clone()))?;

        // In test mode, we don't want to fail if the recorder is already set,
        // and we don't need the global recorder interaction as much.
        let metric_handle = if perform_checks {
            PrometheusBuilder::new().install_recorder().map_err(|e| {
                MyError::Message(format!("Failed to install Prometheus recorder: {e}"))
            })?
        } else {
            let recorder = PrometheusBuilder::new().build_recorder();
            recorder.handle()
        };

        if perform_checks {
            run_startup_checks(config).await?;
        }

        let mut stores = HashMap::new();
        for store_def in &config.cache.stores {
            let cache: Arc<dyn Cache + Send + Sync> = match &store_def.store_type {
                crate::config::StoreType::InMemory => {
                    Arc::new(InMemoryCache::new(Box::new(cache_size.clone())))
                }
                crate::config::StoreType::Redis(redis_config) => {
                    Arc::new(RedisCache::new(redis_config).await?)
                }
                crate::config::StoreType::Mongo(mongo_config) => {
                    Arc::new(MongoCache::new(mongo_config).await?)
                }
                crate::config::StoreType::Oracle(oracle_config) => {
                    Arc::new(OracleCache::new(oracle_config)?)
                }
                crate::config::StoreType::Postgres(pg_config) => {
                    Arc::new(PostgresCache::new(pg_config).await?)
                }
            };
            stores.insert(store_def.name.clone(), cache);
        }

        let mut schema_to_store = HashMap::new();
        for store_def in &config.cache.stores {
            if let Some(schemas) = &store_def.schemas {
                for schema in schemas {
                    schema_to_store.insert(schema.clone(), store_def.name.clone());
                }
            }
        }

        let schema_cache = Arc::new(DashMap::new());

        if let Some(preload_ids) = &config.kafka.preload_schemas {
            let registry_url = config.kafka.schema_registry_url.as_str();
            for id in preload_ids {
                match crate::kafka_utils::fetch_schema_by_id(registry_url, *id).await {
                    Ok(schema) => {
                        info!("Preloaded schema ID: {}", id);
                        schema_cache.insert(*id, schema);
                    }
                    Err(e) => {
                        error!("Failed to preload schema ID {}: {}", id, e);
                        return Err(e);
                    }
                }
            }
        }

        Ok(MyState {
            config: config.clone(),
            stores: Arc::new(stores),
            routes: config.cache.routes.clone(),
            schema_to_store: Arc::new(schema_to_store),
            schema_cache,

            startup_lag_cleared: Arc::new(AtomicBool::new(false)),

            requests_total: Box::new(requests_total),
            requests_miss: Box::new(requests_miss),
            updates_received: Box::new(updates_received),
            tombstones_processed: Box::new(tombstones_processed),
            schema_mismatch_count: Box::new(schema_mismatch_count),
            schema_unrouted_count: Box::new(schema_unrouted_count),
            cache_size: Box::new(cache_size),
            consumer_lag: Box::new(consumer_lag),

            registry,
            prometheus_handle: Arc::new(metric_handle),
        })
    }
}

pub fn service_start(config: &MyConfig) -> Result<(), MyError> {
    let ct = CancellationToken::new();

    run_in_tokio(&config.runtime, service_cancellable(ct, config))
}

pub async fn service_cancellable(ct: CancellationToken, config: &MyConfig) -> Result<(), MyError> {
    let state = MyState::new(config).await?;

    // Initialise liveness here

    let mut config = state.config.hams.clone();

    config.name = NAME.to_owned();
    config.version = VERSION.to_owned();

    // Wrapper to allow sending Hams across threads resulting from spawn_blocking
    // This is necessary because Hams potentially contains raw pointers which are !Send
    struct SendHams(Hams);
    unsafe impl Send for SendHams {}

    let (hams_wrapper, lag_probe) = tokio::task::spawn_blocking(move || {
        let mut hams = Hams::new(config);

        let lag_probe = ProbeManual::new("lag-cleared", false);
        hams.ready_insert(Box::new(FFIProbe::from(lag_probe.clone())) as Box<dyn AsyncHealthProbe>);
        hams.startup_insert(
            Box::new(FFIProbe::from(lag_probe.clone())) as Box<dyn AsyncHealthProbe>
        );
        Ok::<_, MyError>((SendHams(hams), lag_probe))
    })
    .await
    .map_err(|e| MyError::Message(format!("Tokio join error: {}", e)))??;

    let mut hams = hams_wrapper.0;

    hams.register_prometheus(
        // prometheus_response,
        prometheus_response_mystate,
        prometheus_response_free,
        &state as *const _ as *const c_void,
    )?;

    hams.start().unwrap();

    // if state.config.kafka.force_reset_earliest {
    //     crate::kafka_utils::reset_consumer_offsets(&state.config.kafka).await?;
    // }

    // Start Kafka Consumer
    let consumer_state = state.clone();
    let safe_probe = lag_probe.clone();
    tokio::spawn(async move { consumer::start_consumer(consumer_state, safe_probe).await });

    let server = start_app_api(state.clone(), ct.clone());

    server.await?;

    hams.stop()?;
    hams.deregister_prometheus()?;

    ct.cancel();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StartupCheckConfig;
    use crate::startup_tools::run_check;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[tokio::test]
    async fn test_run_check_success_first_try() {
        let config = StartupCheckConfig {
            fails: 3,
            timeout: Duration::from_millis(1),
            enabled: false,
        };

        let result = run_check("test_check".to_string(), &config, || async {
            Ok::<u32, MyError>(42)
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_run_check_retry_success() {
        let config = StartupCheckConfig {
            fails: 3,
            timeout: Duration::from_millis(1),
            enabled: false,
        };

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let result = run_check("test_check_retry".to_string(), &config, || {
            let counter = counter_clone.clone();
            async move {
                let mut c = counter.lock().unwrap();
                *c += 1;
                if *c < 2 {
                    Err(MyError::Message("fail".to_string()))
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(*counter.lock().unwrap(), 2);
    }

    #[tokio::test]
    async fn test_run_check_failure_max_retries() {
        let config = StartupCheckConfig {
            fails: 3,
            timeout: Duration::from_millis(1),
            enabled: false,
        };

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let result: Result<u32, MyError> =
            run_check("test_check_fail".to_string(), &config, || {
                let counter = counter_clone.clone();
                async move {
                    let mut c = counter.lock().unwrap();
                    *c += 1;
                    Err(MyError::Message("always fail".to_string()))
                }
            })
            .await;

        assert!(result.is_err());
        assert_eq!(*counter.lock().unwrap(), 3);
    }
}
