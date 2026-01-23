use std::{ffi::c_void, sync::Arc};

use axum_prometheus::metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use hamsrs::Hams;
use prometheus::{IntGauge, Registry};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    config::MyConfig, error::MyError, tokio_tools::run_in_tokio, webserver::start_app_api,
};

use metrics::{prometheus_response_free, prometheus_response_mystate};

pub mod config;
pub mod error;
pub mod hams;
mod metrics;
pub mod tokio_tools;
pub mod webserver;

/// Name of the Crate
pub const NAME: &str = env!("CARGO_PKG_NAME");
/// Version of the Crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct MyState {
    config: MyConfig,
    pub count_good: Arc<Mutex<usize>>,
    pub count_fail: Arc<Mutex<usize>>,
    registry: Registry,
    prometheus_handle: Arc<PrometheusHandle>,
}

impl MyState {
    pub async fn new(config: &MyConfig) -> Result<MyState, MyError> {
        let registry = Registry::new();

        let hello_counter = IntGauge::new("my_counter", "A counter for my application")?;
        registry.register(Box::new(hello_counter.clone()))?;

        let metric_handle = PrometheusBuilder::new().install_recorder().unwrap();

        Ok(MyState {
            config: config.clone(),
            count_good: Arc::new(Mutex::new(0)),
            count_fail: Arc::new(Mutex::new(0)),
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

    let server = start_app_api(state.clone(), ct.clone());

    server.await?;

    hams.stop()?;
    hams.deregister_prometheus()?;

    ct.cancel();

    Ok(())
}
