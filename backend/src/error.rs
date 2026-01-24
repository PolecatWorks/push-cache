//! Provide a custom error struct
//!
//! Allow derriving MyError from other Error types from dependant packages.

use std::io;

use hamsrs::hamserror::HamsError;
// use hamsrs::hamserror::HamsError;
use axum::extract::rejection::JsonRejection;
use thiserror::Error;
use tracing_subscriber::filter::FromEnvError;

/// Error type for handling errors on Sample
#[derive(Error, Debug)]
pub enum MyError {
    #[error("General error `{0}`")]
    Message(String),
    #[error("Service Cancelled")]
    Cancelled,

    #[error("HaMs error `{0}`")]
    HamsError(#[from] HamsError),

    #[error("Prometheus error `{0}`")]
    PrometheusError(#[from] prometheus::Error),

    #[error("Serdes error `{0}`")]
    Serde(#[from] serde_json::Error),
    #[error("data store disconnected")]
    Io(#[from] io::Error),

    #[error("Json Rejection `{0}`")]
    JsonRejection(#[from] JsonRejection),

    #[error("Shutdown error")]
    ShutdownCheck,

    #[error("PreFlight error")]
    PreflightCheck,

    #[error("Figment error `{0}`")]
    FigmentError(#[from] figment::error::Error),

    #[error("EnvFilter error `{0}`")]
    EnvFilterError(#[from] FromEnvError),

    #[error("Not Found: `{0}`")]
    NotFound(String),

    #[error("Schema Mismatch: expected {expected}, actual {actual}")]
    SchemaMismatch { expected: i32, actual: i32 },
}
