use std::{ffi::c_void, sync::Arc};

use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use dashmap::DashMap;
use hamsrs::Hams;
use prometheus::{IntCounter, Registry};
use rdkafka::consumer::Consumer;
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::{
    config::MyConfig, error::MyError, model::Customer, tokio_tools::run_in_tokio,
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

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(requests_miss.clone()))?;
        registry.register(Box::new(updates_received.clone()))?;
        registry.register(Box::new(tombstones_processed.clone()))?;
        registry.register(Box::new(schema_mismatch_count.clone()))?;

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
            // --- Boot-time Connectivity Checks & Schema Fetch ---
            info!("Performing boot-time connectivity checks...");

            // 1. Kafka Metadata Check
            let kafka_config = &config.kafka;
            let consumer: rdkafka::consumer::BaseConsumer = rdkafka::config::ClientConfig::new()
                .set("bootstrap.servers", &kafka_config.brokers)
                .create()
                .map_err(|e| MyError::Message(format!("Failed to create Kafka client: {}", e)))?;

            // Simple metadata fetch with timeout
            let _metadata = tokio::task::spawn_blocking(move || {
                consumer.fetch_metadata(None, std::time::Duration::from_secs(5))
            })
            .await
            .map_err(|e| MyError::Message(format!("Join error: {}", e)))?
            .map_err(|e| MyError::Message(format!("Failed to fetch Kafka metadata: {}", e)))?;
            info!("Kafka connectivity check passed.");

            // 2. Schema Registry Check & ID Fetch
            let sr_url = kafka_config.schema_registry_url.clone();
            let topic = kafka_config.topic.clone();
            // Assuming TopicNameStrategy for value: <topic>-value
            let subject = format!("{}-value", topic);

            let url = format!("{}/subjects/{}/versions/latest", sr_url, subject);

            let id = reqwest::get(&url)
                .await
                .map_err(|e| {
                    MyError::Message(format!(
                        "Failed to connect to Schema Registry at {}: {}",
                        url, e
                    ))
                })?
                .json::<Value>()
                .await
                .map_err(|e| {
                    MyError::Message(format!("Failed to parse Schema Registry response: {}", e))
                })?
                .get("id")
                .and_then(|id| id.as_u64())
                .ok_or_else(|| MyError::Message("Schema ID not found in response".to_string()))?
                as u32;

            info!("Schema Registry check passed. Expected Schema ID: {}", id);
            id
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
    tokio::spawn(async move {
        consumer::start_consumer(consumer_state).await;
    });

    let server = start_app_api(state.clone(), ct.clone());

    server.await?;

    hams.stop()?;
    hams.deregister_prometheus()?;

    ct.cancel();

    Ok(())
}
