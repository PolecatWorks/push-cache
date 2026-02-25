# PRD: Observability (Java)

## 1. Introduction
The Java implementation provides observability through a dual-mechanism approach: standard Spring Boot Actuator endpoints for deep introspection, and a custom "Hams" service that mimics the Rust sidecar's health check interface on a separate port.

## 2. Goals
- Expose a dedicated HTTP port for health checks (Hams) to match Rust deployment patterns.
- Expose application metrics via Micrometer (Prometheus format).
- Track Kafka consumer lag and cache size.
- Provide health indicators for Liveness and Readiness.

## 3. User Stories

### US-001: Hams Health Server
**Description:** As a platform engineer, I expect a separate health server on a configurable port (e.g., 8079).

**Acceptance Criteria:**
- [ ] Start a raw `com.sun.net.httpserver.HttpServer` on `hams.address`.
- [ ] Expose `/alive` -> Returns 200 OK "OK".
- [ ] Expose `/ready` -> Returns 200 OK "Ready".
- [ ] Expose `/startup` -> Returns 200 OK "Startup OK".
- [ ] Expose `/metrics` -> Returns a placeholder/static response (currently).
- [ ] **Note**: This server currently returns static responses and does not query internal health indicators.

### US-002: Application Metrics (Micrometer)
**Description:** As a developer, I want to track specific application metrics using the standard Spring registry.

**Acceptance Criteria:**
- [ ] Use `MetricsService` to register counters and gauges.
- [ ] `updates_received` (Counter).
- [ ] `tombstones_processed` (Counter).
- [ ] `schema_mismatch_count` (Counter).
- [ ] `schema_unrouted_count` (Counter).
- [ ] `push_cache_consumer_lag_total` (Gauge).
- [ ] `requests_total` (Counter, tagged by `store_name`).
- [ ] `requests_miss` (Counter, tagged by `store_name`).
- [ ] `push_cache_records_total` (Gauge, tagged by `store_name`).

### US-003: Spring Actuator Integration
**Description:** As an operator, I want to use standard Spring Actuator endpoints for debugging.

**Acceptance Criteria:**
- [ ] Enable `spring-boot-starter-actuator`.
- [ ] Enable `micrometer-registry-prometheus`.
- [ ] Expose `/actuator/health` (aggregates `LagClearedHealthIndicator` and `CacheHealthIndicator`).
- [ ] Expose `/actuator/prometheus` (serves the actual metrics gathered by `MetricsService`).

### US-004: Lag Tracking Indicator
**Description:** As a system, I want the Readiness status to reflect Kafka lag.

**Acceptance Criteria:**
- [ ] Implement `LagClearedHealthIndicator`.
- [ ] Status is DOWN until lag clears.
- [ ] `KafkaConsumerService` updates this indicator once lag reaches 0.

## 4. Functional Requirements

### Hams Service
1.  **Implementation**: Manual `HttpServer` to avoid port conflicts with the main Tomcat server and to provide a lightweight sidecar interface.
2.  **Preflight/Shutdown Checks**:
    *   Iterate URLs from config.
    *   Perform HTTP GET requests to external dependencies.
    *   Retry logic with timeout.

### Metrics Service
3.  **Library**: `Micrometer`.
4.  **Registry**: Inject `MeterRegistry`.
5.  **Tagging**: Cache-specific metrics must use the `store_name` tag.

### Health Indicators
6.  **Lag**: `LagClearedHealthIndicator` (AtomicBoolean).
7.  **Cache**: `CacheHealthIndicator` (Iterates all stores, calls `checkHealth()`).

## 5. Non-Goals
- Full parity between the manual Hams server and the Spring Actuator endpoints. (Hams is currently a simplified facade).

## 6. Technical Considerations
- **Dual Ports**: The application opens two ports: one for the main API (Tomcat, 8080) and one for Hams (HttpServer, 8079).
- **Metric Exposure**: Real metrics are available on the *main* port (`/actuator/prometheus`), while the Hams port serves a placeholder. This is a divergence from the Rust implementation where Hams serves the real metrics.

## 7. Success Metrics
- Hams server starts on the configured port.
- Metrics are correctly incremented during ingestion/API usage.
- Actuator Health endpoint reflects the Lag status.
