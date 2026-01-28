use std::{collections::HashSet, ffi::c_void, sync::Arc};

use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use dashmap::DashMap;

use hamsrs::Hams;
use prometheus::{IntCounter, IntGauge, Registry};

use tokio_util::sync::CancellationToken;

use crate::{
    config::MyConfig, error::MyError, kafka_utils::get_schema_id, model::Customer,
    tokio_tools::run_in_tokio, webserver::start_app_api,
};

use metrics::{prometheus_response_free, prometheus_response_mystate};

use crate::startup_tools::run_startup_checks;

pub mod config;
pub mod consumer;
pub mod error;
pub mod hams;
mod kafka_utils;
mod metrics;
pub mod model;
mod startup_tools;
pub mod tokio_tools;
pub mod webserver;

/// Name of the Crate
pub const NAME: &str = env!("CARGO_PKG_NAME");
/// Version of the Crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct MyState {
    config: MyConfig,
    pub cache: Arc<DashMap<String, Customer>>,
    // Metrics
    pub requests_total: Box<IntCounter>,
    pub requests_miss: Box<IntCounter>,
    pub updates_received: Box<IntCounter>,
    pub tombstones_processed: Box<IntCounter>,
    pub schema_mismatch_count: Box<IntCounter>,
    pub cache_size: Box<IntGauge>,
    pub consumer_lag: Box<IntGauge>,
    pub valid_schema_ids: Vec<u32>,

    registry: Registry,
    prometheus_handle: Arc<PrometheusHandle>,
}

impl MyState {
    pub async fn new(config: &MyConfig, perform_checks: bool) -> Result<MyState, MyError> {
        let registry = Registry::new();

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
        let cache_size = IntGauge::new("push_cache_records_total", "Total records in cache")?;
        let consumer_lag =
            IntGauge::new("push_cache_consumer_lag_total", "Total Kafka consumer lag")?;

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(requests_miss.clone()))?;
        registry.register(Box::new(updates_received.clone()))?;
        registry.register(Box::new(tombstones_processed.clone()))?;
        registry.register(Box::new(schema_mismatch_count.clone()))?;
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

        let mut valid_schema_ids = HashSet::new();

        if perform_checks {
            let schema_id = get_schema_id::<Customer>(
                config
                    .kafka
                    .schema_registry_url
                    .as_str()
                    .trim_end_matches('/'), // trim trailing slash
                &config.kafka.topic,
            )
            .await?;

            valid_schema_ids.insert(schema_id.0);
        } else {
            // In test/no-check mode, assume a dummy schema ID if needed, or bypass check
            valid_schema_ids.insert(0);
        }

        let valid_schema_ids_vec: Vec<u32> = valid_schema_ids.into_iter().collect();

        if valid_schema_ids_vec.is_empty() {
            return Err(MyError::Message(
                "No valid schema IDs found. Cannot start consumer.".to_string(),
            ));
        }

        Ok(MyState {
            config: config.clone(),
            cache: Arc::new(DashMap::new()),
            requests_total: Box::new(requests_total),
            requests_miss: Box::new(requests_miss),
            updates_received: Box::new(updates_received),
            tombstones_processed: Box::new(tombstones_processed),
            schema_mismatch_count: Box::new(schema_mismatch_count),
            cache_size: Box::new(cache_size),
            consumer_lag: Box::new(consumer_lag),
            valid_schema_ids: valid_schema_ids_vec,

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
    let state = MyState::new(config, true).await?;

    // Initialise liveness here

    let mut config = state.config.hams.clone();

    config.name = NAME.to_owned();
    config.version = VERSION.to_owned();

    let hams = Hams::new(ct.clone(), &config).unwrap();

    hams.register_prometheus(
        // prometheus_response,
        prometheus_response_mystate,
        prometheus_response_free,
        &state as *const _ as *const c_void,
    )?;

    hams.start().unwrap();

    // Start Kafka Consumer
    let consumer_state = state.clone();
    tokio::spawn(async move { consumer::start_consumer(consumer_state).await });

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
        };

        let result = run_check("test_check", &config, || async { Ok::<u32, MyError>(42) }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_run_check_retry_success() {
        let config = StartupCheckConfig {
            fails: 3,
            timeout: Duration::from_millis(1),
        };

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let result = run_check("test_check_retry", &config, || {
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
        };

        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        let result: Result<u32, MyError> = run_check("test_check_fail", &config, || {
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
