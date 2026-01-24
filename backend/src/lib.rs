use std::{ffi::c_void, sync::Arc};

use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use dashmap::DashMap;
use hamsrs::Hams;
use prometheus::{IntCounter, Registry};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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

    registry: Registry,
    prometheus_handle: Arc<PrometheusHandle>,
}

impl MyState {
    pub async fn new(config: &MyConfig) -> Result<MyState, MyError> {
        let registry = Registry::new();

        let requests_total = IntCounter::new("requests_total", "Total user info requests")?;
        let requests_miss =
            IntCounter::new("requests_miss", "Total requests with no record found")?;
        let updates_received =
            IntCounter::new("updates_received", "Total updates received from Kafka")?;
        let tombstones_processed =
            IntCounter::new("tombstones_processed", "Total tombstone records processed")?;

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(requests_miss.clone()))?;
        registry.register(Box::new(updates_received.clone()))?;
        registry.register(Box::new(tombstones_processed.clone()))?;

        let metric_handle = PrometheusBuilder::new().install_recorder().unwrap();

        Ok(MyState {
            config: config.clone(),
            cache: Arc::new(DashMap::new()),
            requests_total: Box::new(requests_total),
            requests_miss: Box::new(requests_miss),
            updates_received: Box::new(updates_received),
            tombstones_processed: Box::new(tombstones_processed),

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
