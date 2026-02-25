# PRD: Observability (Metrics & Health)

## 1. Introduction
The observability stack ensures the application is monitorable in production. It exposes health checks (liveness/readiness) and Prometheus-compatible metrics to track performance, cache efficiency, and data ingestion lag.

## 2. Goals
- Expose a dedicated HTTP port for health and metrics (separate from the main API).
- Provide Liveness and Readiness probes for Kubernetes.
- Expose detailed application metrics (cache hits/misses, ingestion lag, etc.).
- Ensure metrics aggregation from multiple sources (Application logic + Web server).

## 3. User Stories

### US-001: Health Check Endpoint
**Description:** As a platform engineer, I need a health check endpoint to know if the service is running.

**Acceptance Criteria:**
- [ ] Expose health endpoints on a configured address (default `0.0.0.0:8079`).
- [ ] `/health/live` returns 200 OK if the process is running.
- [ ] `/health/ready` returns 200 OK only when the application is fully initialized and critical dependencies are healthy.

### US-002: Readiness Probe - Consumer Lag
**Description:** As a platform engineer, I want the service to be "Not Ready" until it has caught up with the Kafka topic.

**Acceptance Criteria:**
- [ ] Implement a `lag-cleared` probe.
- [ ] Probe starts as `false` (Not Ready).
- [ ] When Kafka consumer lag reaches 0 (or near 0), probe flips to `true`.
- [ ] Readiness endpoint returns 503 until lag is cleared.

### US-003: Prometheus Metrics Endpoint
**Description:** As a site reliability engineer, I want to scrape metrics to visualize system performance.

**Acceptance Criteria:**
- [ ] Expose `/metrics` endpoint on the health port.
- [ ] Return metrics in Prometheus text format.
- [ ] Include standard process metrics (CPU, RAM).
- [ ] Include application-specific metrics.

### US-004: Application Metrics
**Description:** As a developer, I want specific metrics to debug cache behavior.

**Acceptance Criteria:**
- [ ] `requests_total`: Counter of total API requests.
- [ ] `requests_miss`: Counter of requests where key was not found.
- [ ] `updates_received`: Counter of Kafka messages processed.
- [ ] `tombstones_processed`: Counter of delete markers processed.
- [ ] `schema_mismatch_count`: Counter of messages with invalid schema/magic byte.
- [ ] `schema_unrouted_count`: Counter of messages with valid schema but no configured store.
- [ ] `push_cache_records_total` (`cache_size`): Gauge of items in the `in_memory` cache.
- [ ] `push_cache_consumer_lag_total`: Gauge of current Kafka consumer lag.

## 4. Functional Requirements

### Health Service (`Hams`)
1.  **Library**: Use `hamsrs` for managing health checks and the administrative server.
2.  **Configuration**:
    *   Address/Port (e.g., 8079).
    *   Logging enabled/disabled.
3.  **Probes**:
    *   `lag-cleared`: Manual probe controlled by the Kafka Consumer logic.

### Metrics Registry
4.  **Libraries**: Use `prometheus` crate for core metrics and `axum-prometheus` for web server metrics.
5.  **Aggregation**: The `/metrics` endpoint must gather data from:
    *   The global/shared `prometheus::Registry` (custom app metrics).
    *   The `axum-prometheus` handle (HTTP request durations, etc.).
6.  **FFI Integration**: Use `hamsrs` FFI callbacks (`prometheus_response_mystate`) to serve the aggregated metrics.

## 5. Non-Goals
- Distributed tracing (e.g., Jaeger/OTEL) is out of scope for this iteration, though `tracing` crate is used for logging.
- Push-gateway support.

## 6. Technical Considerations
- **Unsafe Code**: The integration between `hamsrs` (C-compatible FFI) and the Rust application state requires `unsafe` blocks to pass pointers to the metrics registry.
- **Thread Safety**: Metrics must be thread-safe (`Atomic` or `Mutex` protected) as they are updated by the consumer loop and read by the metrics server.

## 7. Success Metrics
- Prometheus can successfully scrape `/metrics`.
- Kubernetes successfully restarts pods if Liveness fails.
- Kubernetes delays traffic (Service endpoints) until Readiness passes (lag cleared).
