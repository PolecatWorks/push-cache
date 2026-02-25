# Java Backend Specification

This directory contains detailed technical specifications for the `java-container` service, which is a port of the Rust `push-cache` backend.

## Implementation Sequence

To rebuild this Java application, implement the components in this specific order to minimize circular dependencies and ensure a smooth TDD process.

### 1. Configuration (`prd-configuration.md`)
**Why first?** The application relies entirely on the configuration object (`AppConfig`) for feature flags, service addresses, and secrets.
- **Key Output**: A CLI (`picocli`) that loads a valid `AppConfig` from YAML/Env/Secrets.

### 2. Observability (`prd-observability.md`)
**Why second?** Setting up the manual `HamsService` (sidecar health) and Spring Actuator early ensures every subsequent component (Cache, Ingestion) can be instrumented immediately.
- **Key Output**: A dual-port application (Tomcat on 8080, HttpServer on 8079).

### 3. Core Cache (`prd-cache-core.md`)
**Why third?** The `Cache` interface and implementations (`InMemory`, `Redis`) are the core domain objects required by both the Consumer (Write) and API (Read).
- **Key Output**: A `CacheFactory` producing `InMemoryCache` and `RedisCache` beans.

### 4. Ingestion (`prd-ingestion.md`)
**Why fourth?** This layer populates the empty cache stores created in the previous step by consuming from Kafka.
- **Key Output**: A `KafkaConsumerService` thread consuming messages and writing to the cache.

### 5. API Layer (`prd-api.md`)
**Why last?** The API exposes the cached data. It depends on `AppConfig` for routing rules and `CacheFactory` for data access.
- **Key Output**: Functional endpoints (`RouterFunction`) serving JSON data.

---

## File Index
- [Comparison Analysis](./comparison.md) (Differences vs Rust)
- [Configuration](./prd-configuration.md)
- [Observability](./prd-observability.md)
- [Core Cache](./prd-cache-core.md)
- [Ingestion](./prd-ingestion.md)
- [API](./prd-api.md)
