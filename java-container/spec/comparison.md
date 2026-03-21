# Comparison: Java vs Rust Implementation

This document highlights the architectural and implementation differences between the `java-container` (Port) and `rust-container` (Reference) services.

## 1. Observability & Health

### Rust (Reference)
- **Library**: Uses `hamsrs`, a dedicated health and metrics server.
- **Mechanism**:
    - Opens a separate HTTP port (e.g., 8079).
    - Manages a lifecycle of checks: `preflight` -> `startup` -> `ready` -> `live` -> `shutdown`.
    - **Readiness Probe**: The `/ready` endpoint is gated by a `lag-cleared` probe. It returns 503 until the Kafka consumer has caught up (lag = 0).
    - **Metrics**: Serves Prometheus metrics directly from the application registry via FFI.

### Java (Port)
- **Library**: Custom `HamsService` using `com.sun.net.httpserver.HttpServer` + Standard Spring Boot Actuator.
- **Mechanism**:
    - **Manual Sidecar**: The `HamsService` opens a separate port (e.g., 8079) to match the Rust deployment pattern. It serves `/alive`, `/startup`, and `/ready` checks by directly querying the internal `CacheHealthIndicator` and `LagClearedHealthIndicator` beans.
    - **Metrics**: The `/metrics` endpoint serves Prometheus metrics by querying the internal `PrometheusMeterRegistry`.
    - **Actuator**: The main application port (e.g., 8080) also exposes `/actuator/health` and `/actuator/prometheus` for deeper introspection.
- **Parity**: High. The custom `HamsService` correctly bridges the gap to provide the expected sidecar API endpoints, utilizing the internal Spring Boot indicators and registries.

## 2. Concurrency Model

### Rust
- **Async/Await**: Built on `tokio` and `axum`.
- **Non-Blocking**: All IO (Kafka, Redis, HTTP) is asynchronous.
- **Throughput**: Designed for high concurrency with low thread count.

### Java
- **Blocking/Synchronous**: Built on Spring WebMvc (Servlet) and `kafka-clients`.
- **Thread per Request**: The `RecordHandler` and `Cache` interface are synchronous.
- **Kafka**: The consumer runs in a dedicated thread.
- **Impact**: Redis operations (GET/PUT) block the Servlet thread. `getKeys` on Redis uses a cursor but still blocks the calling thread during iteration.

## 3. Configuration Loading

### Rust
- **Tool**: `figment`.
- **Layering**: `YAML File` -> `Secrets Directory` -> `Env Vars (APP_*)`.
- **Structure**: Uses `serde` to map strict types (`UrlWithUsernamePassword`).

### Java
- **Tool**: Spring Boot Configuration.
- **Layering**: `spring.config.additional-location` -> `configtree` -> `Env Vars`.
- **Parity**: High. The Java implementation successfully replicates the Rust layering strategy using standard Spring features.

## 4. Ingestion & Routing

### Rust
- **Client**: `rdkafka` (librdkafka C binding).
- **Schema Registry**: Custom HTTP client implementation (due to library bugs/limitations).
- **Logic**: Manual poll loop, manual commit (configured), manual lag tracking.

### Java
- **Client**: `kafka-clients` (Java native).
- **Schema Registry**: `RestClient` (Spring 6).
- **Logic**: Manual poll loop (in `KafkaConsumerService`), manual lag tracking.
- **Parity**: High. Both implement the "Magic Byte -> Schema ID -> Cache Lookup -> Store Routing" flow identically.

## 5. API Layer

### Rust
- **Framework**: `axum`.
- **Routing**: Dynamic router construction nested under base path.
- **Serialization**: `apache_avro` -> `serde_json::Value` -> HTTP Body.

### Java
- **Framework**: Spring WebMvc (`RouterFunctions`).
- **Routing**: Dynamic `RouterFunction` composition.
- **Serialization**: `apache-avro` (`GenericDatumWriter`) -> JSON Bytes -> HTTP Body.
- **Parity**: High. Both implementations support dynamic routing based on configuration, including the "Get Record by Body" endpoint (`_by_body` suffix) for POST-like retrieval of GET resources using a JSON body.

## 6. Storage Backends (New Divergence)

### Rust
- Supports `InMemory`, `Redis`, `Mongo`, and `Oracle` cache stores.
- The `Mongo` store persists data as BSON documents.
- The `Oracle` store persists data as BLOBs with a VARCHAR2 primary key.

### Java
- Currently supports `IN_MEMORY`, `REDIS`, and `MONGO` cache stores.
- The `MONGO` store persists data as BSON documents matching the Rust implementation.
- **Gap**: The Java implementation needs to add support for the Oracle backend to reach full feature parity.

## 7. Summary of Work Remaining
To achieve full parity, the Java implementation requires:
1.  **Oracle Backend Support**: Implement the `OracleCache` class and backend handling.
2.  **Redis Async**: (Optional) Consider moving to Spring WebFlux if non-blocking Redis access is required for performance parity under load.
