use std::{ffi::c_void, sync::Arc};

use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use dashmap::DashMap;

use hamsrs::Hams;
use prometheus::{IntCounter, IntGauge, Registry};
use rdkafka::{
    ClientConfig,
    consumer::{BaseConsumer, Consumer},
};
use serde_json::Value;

/// Checks if the Schema Registry is reachable and supports the specified schema type.
///
/// # Arguments
///
/// * `config` - The application configuration containing the Schema Registry URL.
/// * `schema_type` - The schema type to check for (e.g., "AVRO", "JSON", "PROTOBUF").
///
/// # Errors
///
/// Returns `MyError` if:
/// * The connection to the Schema Registry fails.
/// * The Schema Registry returns a non-success status code.
/// * The response from the Schema Registry cannot be parsed.
/// * The specified `schema_type` is not supported by the Schema Registry.
async fn check_schema_registry(url: &Url, schema_type: &str) -> Result<(), MyError> {
    let mut sr_url = url.clone();

    sr_url.set_path("/schemas/types");

    let res = reqwest::get(sr_url.clone()).await.map_err(|e| {
        MyError::Message(format!(
            "Failed to connect to Schema Registry at {sr_url}: {e}"
        ))
    })?;

    if !res.status().is_success() {
        return Err(MyError::Message(format!(
            "Schema Registry check failed with status: {}",
            res.status()
        )));
    }
    let schema_types = res
        .json::<Vec<String>>()
        .await
        .map_err(|e| MyError::Message(format!("Failed to parse Schema Registry response: {e}")))?;

    if !schema_types.contains(&schema_type.to_uppercase()) {
        return Err(MyError::Message(format!(
            "Schema type {schema_type} is not supported by the Schema Registry. Supported types: {schema_types:?}"
        )));
    }

    Ok(())
}

/// Checks if the Kafka broker is reachable and the specified topic exists.
///
/// # Arguments
///
/// * `url` - The URL of the Kafka broker.
/// * `config` - The Kafka configuration containing broker and topic details.
///
/// # Errors
///
/// Returns `MyError` if:
/// * The connection to the Kafka broker fails.
/// * The metadata for the specified topic cannot be fetched.
/// * The specified topic does not exist or has no partitions.
async fn check_kafka_metadata(config: &MyKafkaConfig) -> Result<(), MyError> {
    let consumer: BaseConsumer = ClientConfig::new()
        .set(
            "bootstrap.servers",
            format!(
                "{}:{}",
                config
                    .brokers
                    .host_str()
                    .ok_or_else(|| MyError::Message(format!(
                        "Failed to get Kafka broker host {}",
                        config.brokers
                    )))?,
                config
                    .brokers
                    .port()
                    .ok_or_else(|| MyError::Message(format!(
                        "Failed to get Kafka broker port {}",
                        config.brokers
                    )))?,
            ),
        )
        .create()?;

    // Fetch metadata for the specific topic
    // Passing Some(topic_name) limits the request to just that topic

    let metadata = consumer.fetch_metadata(Some(&config.topic), config.fetch_metadata_timeout)?;

    let topics = metadata.topics();
    if !topics
        .iter()
        .any(|t| t.name() == config.topic && t.error().is_none() && !t.partitions().is_empty())
    {
        return Err(MyError::Message(format!(
            "Kafka topic {} not found or has not partitions",
            config.topic
        )));
    }

    Ok(())
}

/// Executes an asynchronous check with a retry mechanism.
///
/// This function repeatedly calls the `make_future` closure to generate and await a future
/// until it succeeds or the maximum number of attempts specified in `config` is reached.
/// It waits for the duration specified in `config.timeout` between failed attempts.
///
/// # Arguments
///
/// * `name` - A descriptive name for the check, used in logging and error messages.
/// * `config` - Configuration defining the number of retries and the timeout between them.
/// * `make_future` - A closure that produces the future to be executed for each attempt.
///
/// # Errors
///
/// Returns `MyError` if the check fails after all configured attempts.
async fn run_check<G, F, T>(
    name: &str,
    config: &StartupCheckConfig,
    mut make_future: G,
) -> Result<T, MyError>
where
    G: FnMut() -> F, // G is a generator that creates futures
    F: Future<Output = Result<T, MyError>>,
{
    info!("Running checks: {:?}", config);

    let mut attempts_remaining = config.fails;

    while attempts_remaining > 0 {
        // Call the closure to get a fresh future instance for this attempt
        if let Ok(reply) = make_future().await {
            info!("Check passed");
            return Ok(reply);
        }

        attempts_remaining -= 1;
        if attempts_remaining > 0 {
            warn!("Check failed: {name}, rerunning in {:?}", config.timeout);
            tokio::time::sleep(config.timeout).await;
        }
    }

    Err(MyError::Message(format!(
        "Check {} failed after {} attempts",
        name, config.fails
    )))
}

/// Fetches the latest schema ID for the given topic from the Schema Registry.
///
/// # Arguments
///
/// * `sr_url` - The base URL of the Schema Registry.
/// * `topic` - The name of the Kafka topic.
///
/// # Errors
///
/// Returns `MyError` if:
/// * The connection to the Schema Registry fails.
/// * The response cannot be parsed or lacks the "id" field.
async fn fetch_latest_schema_id(sr_url: &Url, topic: &str) -> Result<u32, MyError> {
    let subject = format!("{topic}-value");
    let url = format!("{sr_url}/subjects/{subject}/versions/latest");

    let id = reqwest::get(&url)
        .await
        .map_err(|e| {
            MyError::Message(format!(
                "Failed to connect to Schema Registry at {url}: {e}"
            ))
        })?
        .json::<Value>()
        .await
        .map_err(|e| MyError::Message(format!("Failed to parse Schema Registry response: {e}")))?
        .get("id")
        .and_then(|id| id.as_u64())
        .ok_or_else(|| MyError::Message("Schema ID not found in response".to_string()))?
        as u32;

    Ok(id)
}

async fn run_startup_checks(config: &MyConfig) -> Result<u32, MyError> {
    let checks_config = &config.startup_checks;

    // 1. Run connectivity checks (Schema Registry & Kafka) in parallel
    // These checks ensure the services are reachable and basic requirements are met.
    // run_check handles retries internally.
    tokio::try_join!(
        run_check("Schema Registry Connectivity", checks_config, || {
            check_schema_registry(&config.kafka.schema_registry_url, "AVRO")
        }),
        run_check("Kafka Metadata Connectivity", checks_config, || {
            check_kafka_metadata(&config.kafka)
        }),
    )?;

    // 2. Fetch the latest Schema ID
    // This is separated because it depends on the Schema Registry being reachable,
    // and we want to return this value.
    let schema_id = run_check("Schema Registry ID Fetch", checks_config, || {
        fetch_latest_schema_id(&config.kafka.schema_registry_url, &config.kafka.topic)
    })
    .await?;

    info!("All startup checks passed. Schema ID: {}", schema_id);

    Ok(schema_id)
}

use tokio_util::sync::CancellationToken;

// use tokio::time::Duration; // Note: std::time::Duration is already used in the code, so we can use that.
use tracing::{info, warn};
use url::Url;

use crate::{
    config::{MyConfig, MyKafkaConfig, StartupCheckConfig},
    error::MyError,
    model::Customer,
    tokio_tools::run_in_tokio,
    webserver::start_app_api,
};

use metrics::{prometheus_response_free, prometheus_response_mystate};

pub mod config;
pub mod consumer;
pub mod error;
pub mod hams;
mod metrics;
pub mod model;
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
    pub expected_schema_id: u32,

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
                MyError::Message(format!("Failed to install Prometheus recorder: {}", e))
            })?
        } else {
            let recorder = PrometheusBuilder::new().build_recorder();
            recorder.handle()
        };

        let expected_schema_id = if perform_checks {
            run_startup_checks(config).await?
        } else {
            0
        };

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
            expected_schema_id,

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
