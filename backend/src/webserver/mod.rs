// pub mod dependencies;
// pub mod services;
// pub mod users;

use axum::{
    Json, Router,
    extract::{FromRequest, MatchedPath, Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_prometheus::PrometheusMetricLayer;
use reqwest::StatusCode;
use tower_http::trace::{DefaultOnFailure, DefaultOnRequest, DefaultOnResponse, TraceLayer};
use tracing::{Level, info};

use crate::model::Customer;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio_util::sync::CancellationToken;

use crate::{MyState, error::MyError};

#[derive(Deserialize)]
struct ListUsersParams {
    limit: Option<usize>,
    offset: Option<usize>,
    filter: Option<String>,
}

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
            address: "0.0.0.0:8080".parse().unwrap(),
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
        .route("/users", post(create_user).get(list_users))
        .route("/users/{account_id}", get(get_user).delete(delete_user))
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

/// Handler for GET /users/{account_id}
/// Retrieves a customer by their Account ID.
/// Returns 200 OK with the customer data if found, or 404 Not Found.
async fn get_user(
    State(state): State<MyState>,
    Path(account_id): Path<String>,
) -> Result<impl IntoResponse, MyError> {
    // state.requests_total.inc();

    if let Some(customer) = state.cache.get(&account_id) {
        // DashMap's get returns a Ref helper, verify we can serialize it or clone it.
        // Customer implements Clone.
        let customer_data = customer.value().clone();

        // Headers
        let mut headers = HeaderMap::new();
        // Standard caching headers
        let max_age = state.config.kafka.cache_max_age;
        headers.insert(
            "Cache-Control",
            format!("public, max-age={}", max_age.as_secs())
                .parse()
                .unwrap(),
        );
        // ETag could be a hash of the content, or just updated_at timestamp
        headers.insert(
            "ETag",
            format!("\"{}\"", customer_data.updatedAt).parse().unwrap(),
        );

        return Ok((headers, Json(customer_data)));
    }

    state.requests_miss.inc();
    Err(MyError::NotFound("User not found".into()))
}

/// Handler for POST /users
/// Creates a new customer.
/// Returns 201 Created on success, or 409 Conflict if the user already exists.
async fn create_user(
    State(state): State<MyState>,
    Json(customer): Json<Customer>,
) -> Result<impl IntoResponse, MyError> {
    use dashmap::mapref::entry::Entry;

    match state.cache.entry(customer.accountId.clone()) {
        Entry::Occupied(_) => Ok((
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "message": "User already exists" })),
        )),
        Entry::Vacant(entry) => {
            entry.insert(customer.clone());
            state.cache_size.inc();
            Ok((
                StatusCode::CREATED,
                Json(serde_json::to_value(customer).unwrap()),
            ))
        }
    }
}

/// Handler for DELETE /users/{account_id}
/// Deletes a customer by their Account ID.
/// Returns 200 OK with the deleted customer data, or 404 Not Found.
async fn delete_user(
    State(state): State<MyState>,
    Path(account_id): Path<String>,
) -> Result<impl IntoResponse, MyError> {
    if let Some((_, customer)) = state.cache.remove(&account_id) {
        state.cache_size.dec();
        return Ok((StatusCode::OK, Json(customer)));
    }
    Err(MyError::NotFound("User not found".into()))
}

/// Handler for GET /users
/// Lists customer keys with optional filtering and pagination.
/// Returns 200 OK with a list of account IDs.
async fn list_users(
    State(state): State<MyState>,
    Query(params): Query<ListUsersParams>,
) -> Result<impl IntoResponse, MyError> {
    let mut keys: Vec<String> = state
        .cache
        .iter()
        .map(|entry| entry.key().clone())
        .collect();

    // Filter
    if let Some(filter) = &params.filter {
        keys.retain(|k| k.contains(filter));
    }

    // Sort for stability
    keys.sort();

    // Pagination
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(usize::MAX);

    let paged_keys: Vec<String> = keys.into_iter().skip(offset).take(limit).collect();

    Ok(Json(paged_keys))
}

impl IntoResponse for MyError {
    fn into_response(self) -> Response {
        #[derive(Serialize)]
        struct ErrorResponse {
            message: String,
        }

        let (status, message) = match self {
            MyError::Message(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.to_string()),
            MyError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.to_string()),
            MyError::SchemaMismatch { .. } => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Schema Mismatch".to_string(),
            ),
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
            MyError::KafkaError(_error) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Kafka Error".to_string())
            }
        };

        // Use a public constructor or helper function for ErrorResponse.
        // Replace ErrorResponse::new(message) with the correct public API.
        (status, AppJson(ErrorResponse { message })).into_response()
    }
}

// Tests removed as they rely on DB
#[cfg(test)]
mod tests {
    use super::*;
    use crate::MyState;
    use crate::config::{MyConfig, MyKafkaConfig};
    use crate::model::Customer;
    use axum::routing::delete;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };

    use tower::util::ServiceExt; // for oneshot

    async fn get_test_state() -> MyState {
        let kafka_config = MyKafkaConfig {
            brokers: "tcp://localhost:9092".parse().unwrap(),
            group_id: "test".to_string(),
            topic: "test-topic".to_string(),
            schema_registry_url: "http://localhost:8081".parse().unwrap(),
            cache_max_age: std::time::Duration::from_secs(60),
            fetch_metadata_timeout: std::time::Duration::from_secs(5),
            offset_reset: crate::config::KafkaOffsetReset::Earliest,
        };

        let config = MyConfig {
            hams: hamsrs::hams::config::HamsConfig::default(),
            runtime: crate::tokio_tools::ThreadRuntime {
                threads: 1,
                stack_size: 1024 * 1024,
                name: "test".to_string(),
            },
            webservice: WebServiceConfig {
                prefix: "/api".to_string(),
                address: "0.0.0.0:8080".parse().unwrap(),
                forwarding_headers: vec![],
            },
            kafka: kafka_config,
            startup_checks: crate::config::StartupCheckConfig {
                fails: 1,
                timeout: std::time::Duration::from_millis(100),
            },
        };

        MyState::new(&config, false).await.unwrap()
    }

    #[tokio::test]
    async fn test_create_user_success() {
        let state = get_test_state().await;

        let app = Router::new()
            .route("/users", post(create_user))
            .with_state(state.clone());

        let customer = Customer {
            accountId: "new_user".to_string(),
            name: "New User".to_string(),
            address: "New Address".to_string(),
            phone: "000".to_string(),
            createdAt: 100,
            updatedAt: 200,
        };

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&customer).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        assert!(state.cache.contains_key("new_user"));
    }

    #[tokio::test]
    async fn test_create_user_conflict() {
        let state = get_test_state().await;
        // Pre-populate
        let customer = Customer {
            accountId: "existing".to_string(),
            name: "Existing".to_string(),
            address: "Address".to_string(),
            phone: "123".to_string(),
            createdAt: 100,
            updatedAt: 200,
        };
        state
            .cache
            .insert(customer.accountId.clone(), customer.clone());

        let app = Router::new()
            .route("/users", post(create_user))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/users")
                    .header("Content-Type", "application/json")
                    .body(Body::from(serde_json::to_vec(&customer).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_delete_user_success() {
        let state = get_test_state().await;
        let customer = Customer {
            accountId: "to_delete".to_string(),
            name: "Delete Me".to_string(),
            address: "Address".to_string(),
            phone: "123".to_string(),
            createdAt: 100,
            updatedAt: 200,
        };
        state.cache.insert("to_delete".to_string(), customer);

        let app = Router::new()
            .route("/users/{account_id}", delete(delete_user))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/users/to_delete")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(!state.cache.contains_key("to_delete"));
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let state = get_test_state().await;
        let app = Router::new()
            .route("/users/{account_id}", delete(delete_user))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/users/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_list_users() {
        let state = get_test_state().await;
        state.cache.insert(
            "user1".to_string(),
            Customer {
                accountId: "user1".to_string(),
                name: "User 1".to_string(),
                address: "A".to_string(),
                phone: "1".to_string(),
                createdAt: 0,
                updatedAt: 0,
            },
        );
        state.cache.insert(
            "user2".to_string(),
            Customer {
                accountId: "user2".to_string(),
                name: "User 2".to_string(),
                address: "A".to_string(),
                phone: "1".to_string(),
                createdAt: 0,
                updatedAt: 0,
            },
        );

        let app = Router::new()
            .route("/users", get(list_users))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/users")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let keys: Vec<String> = serde_json::from_slice(&body_bytes).unwrap();

        // Sorting is guaranteed by implementation
        assert_eq!(keys, vec!["user1", "user2"]);
    }

    #[tokio::test]
    async fn test_list_users_pagination() {
        let state = get_test_state().await;
        for i in 0..5 {
            let id = format!("user{}", i);
            state.cache.insert(
                id.clone(),
                Customer {
                    accountId: id,
                    name: "U".to_string(),
                    address: "A".to_string(),
                    phone: "1".to_string(),
                    createdAt: 0,
                    updatedAt: 0,
                },
            );
        }

        let app = Router::new()
            .route("/users", get(list_users))
            .with_state(state.clone());

        // Limit 2, Offset 1 -> user1, user2 (user0, user1, user2, user3, user4 sorted)
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/users?limit=2&offset=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let keys: Vec<String> = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(keys, vec!["user1", "user2"]);
    }

    #[tokio::test]
    async fn test_list_users_filter() {
        let state = get_test_state().await;
        state.cache.insert(
            "apple".to_string(),
            Customer {
                accountId: "apple".to_string(),
                name: "".to_string(),
                address: "".to_string(),
                phone: "".to_string(),
                createdAt: 0,
                updatedAt: 0,
            },
        );
        state.cache.insert(
            "banana".to_string(),
            Customer {
                accountId: "banana".to_string(),
                name: "".to_string(),
                address: "".to_string(),
                phone: "".to_string(),
                createdAt: 0,
                updatedAt: 0,
            },
        );
        state.cache.insert(
            "apricot".to_string(),
            Customer {
                accountId: "apricot".to_string(),
                name: "".to_string(),
                address: "".to_string(),
                phone: "".to_string(),
                createdAt: 0,
                updatedAt: 0,
            },
        );

        let app = Router::new()
            .route("/users", get(list_users))
            .with_state(state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/users?filter=ap")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let keys: Vec<String> = serde_json::from_slice(&body_bytes).unwrap();

        assert_eq!(keys, vec!["apple", "apricot"]);
    }

    #[tokio::test]
    async fn test_get_user_found() {
        let state = get_test_state().await;

        // Populate cache
        let customer = Customer {
            accountId: "123".to_string(),
            name: "Test User".to_string(),
            address: "Address".to_string(),
            phone: "123".to_string(),
            createdAt: 100,
            updatedAt: 200,
        };
        state.cache.insert("123".to_string(), customer);

        let app = Router::new()
            .route("/users/{account_id}", get(get_user))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/users/123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Verify Headers
        let headers = response.headers();
        assert!(headers.contains_key("cache-control"));
        assert!(headers.contains_key("etag"));
        assert_eq!(headers["cache-control"], "public, max-age=60");
        assert_eq!(headers["etag"], "\"200\"");
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let state = get_test_state().await;

        let app = Router::new()
            .route("/users/{account_id}", get(get_user))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/users/999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
