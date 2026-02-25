# Comparison: Java vs Rust Implementation

This document highlights the architectural and implementation differences between the `java-container` (Port) and `rust-container` (Reference) services.

## 1. Observability & Health (Major Divergence)

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
    - **Manual Sidecar**: The `HamsService` opens a separate port (e.g., 8079) but serves **static** responses for `/ready` and `/alive`. It **does not** currently check the `LagClearedHealthIndicator`.
    - **Actuator**: The main application port (e.g., 8080) exposes `/actuator/health` and `/actuator/prometheus`. These **do** contain the correct logic (Lag check and Metrics), but they are on the "wrong" port compared to the Rust sidecar pattern.
- **Gap**: The Java `HamsService` needs to be updated to query the internal `HealthIndicator` beans instead of returning static strings, to truly match the Rust behavior on the sidecar port.

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
- **Divergence**: Rust supports `preload_schemas` configuration to fetch specific schema IDs at startup. Java does not yet implement this.

## 5. API Layer

### Rust
- **Framework**: `axum`.
- **Routing**: Dynamic router construction nested under base path.
- **Serialization**: `apache_avro` -> `serde_json::Value` -> HTTP Body.

### Java
- **Framework**: Spring WebMvc (`RouterFunctions`).
- **Routing**: Dynamic `RouterFunction` composition.
- **Serialization**: `apache-avro` (`GenericDatumWriter`) -> JSON Bytes -> HTTP Body.
- **Parity**: Medium. Java lacks the "Get Record by Body" endpoint (`_by_body` suffix) which was added to Rust for POST-like retrieval of GET resources using a JSON body.

## 6. Summary of Work Remaining
To achieve full parity, the Java implementation requires:
1.  **Hams Upgrade**: Connect the manual `HamsService` endpoints (`/ready`, `/metrics`) to the internal Spring `HealthIndicator` and `MeterRegistry` beans so the sidecar port reports actual status.
2.  **Redis Async**: (Optional) Consider moving to Spring WebFlux if non-blocking Redis access is required for performance parity under load.
