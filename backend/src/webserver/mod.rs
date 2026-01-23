// pub mod dependencies;
// pub mod services;
// pub mod users;

use axum::{
    Router,
    extract::{FromRequest, MatchedPath},
    response::{IntoResponse, Response},
    routing::get,
};
use axum_prometheus::PrometheusMetricLayer;
use reqwest::StatusCode;
use tower_http::trace::{DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use crate::{MyState, error::MyError};

/// Service Configuration
#[derive(Deserialize, Debug, Clone)]
pub struct WebServiceConfig {
    /// Prefix of the served API
    pub prefix: String,
    /// Hostname to start the webservice on
    pub address: SocketAddr,
    pub forwarding_headers: Vec<String>,
}

impl Default for WebServiceConfig {
    fn default() -> Self {
        Self {
            prefix: "/api".to_string(),
            address: "127.0.0.1:8080".parse().unwrap(),
            forwarding_headers: vec![],
        }
    }
}

// // Handler for POST /messages
// async fn create_message(Json(message): Json<Message>) -> impl IntoResponse {
//     info!("Handling create_message request");
//     Json(format!("New message: {}", message.content))
// }

#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(MyError))]
pub struct AppJson<T>(T);

impl<T> IntoResponse for AppJson<T>
where
    axum::Json<T>: IntoResponse,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

pub async fn start_app_api(state: MyState, ct: CancellationToken) -> Result<(), MyError> {
    let prefix = state.config.webservice.prefix.clone();

    let shared_state = state.clone();

    let metric_layer = PrometheusMetricLayer::new();

    // Setup http server
    let app = Router::new()
        // .nest("/users", users::user_apis())
        // .nest("/services", services::service_apis())
        // .nest("/dependencies", dependencies::dependency_apis())
        .route("/hello", get(|| async { "Hello, World!" }))
        // .route("/metrics", get(|| async move { metric_handle.render() }))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
                    let matched_path = request
                        .extensions()
                        .get::<MatchedPath>()
                        .map(|matched_path| matched_path.as_str());

                    tracing::info_span!(
                        "request",
                        method = ?request.method(),
                        uri = ?request.uri(),
                        matched_path = ?matched_path,
                    )
                })
                .on_request(DefaultOnRequest::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
                .on_failure(DefaultOnFailure::new().level(Level::ERROR)),
        )
        .layer(metric_layer)
        .with_state(shared_state);

    let prefix_app = Router::new().nest(&prefix, app);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind(state.config.webservice.address).await?;
    let server = axum::serve(listener, prefix_app).with_graceful_shutdown(async move {
        // The move is necessary as with_graceful_shutdown requires static lifetime
        ct.cancelled().await
    });

    info!(
        "Server started on {}{prefix}",
        state.config.webservice.address
    );

    Ok(server.await?)
}

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            MyError::Message(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            MyError::Cancelled => (StatusCode::INTERNAL_SERVER_ERROR, "Cancelled".to_string()),
            MyError::HamsError(_error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Hams Error".to_string())
            }
            MyError::Serde(_error) => (StatusCode::BAD_REQUEST, "Serde Error".to_string()),
            MyError::Io(_error) => (StatusCode::INTERNAL_SERVER_ERROR, "IO Error".to_string()),
            MyError::ShutdownCheck => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Shutdown Check Failed".to_string(),
            ),
            MyError::PreflightCheck => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Preflight Check Failed".to_string(),
            ),
            MyError::FigmentError(_error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Config Error".to_string(),
            ),
            MyError::JsonRejection(rejection) => (rejection.status(), rejection.body_text()),
            MyError::PrometheusError(_error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Prometheus Error".to_string(),
            ),
            MyError::EnvFilterError(_error) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "EnvFilter Error".to_string(),
            ),
        };

        // Use a public constructor or helper function for ErrorResponse.
        // Replace ErrorResponse::new(message) with the correct public API.
        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

// Tests removed as they rely on DB
