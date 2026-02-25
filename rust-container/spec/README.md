# Rust Backend Specification

This directory contains detailed technical specifications for the `push-cache` Rust service. These documents are designed to guide the development of the application in a modular, test-driven manner.

## Implementation Sequence

To build this application from scratch, follow the PRDs in this specific order. Each component builds upon the previous one.

### 1. Configuration (`prd-configuration.md`)
**Why first?** Defines the application structure, CLI arguments, and how secrets/env vars are loaded. Every other component relies on the `MyConfig` struct.
- **Key Output**: A working CLI that parses args and loads a valid configuration object.

### 2. Observability (`prd-observability.md`)
**Why second?** Establishing metrics and health checks early allows all subsequent components (Cache, Consumer, API) to be instrumented immediately as they are built.
- **Key Output**: A `Hams` server running on a separate port exposing `/health` and `/metrics`.

### 3. Core Cache (`prd-cache-core.md`)
**Why third?** The core data structures must exist before we can write to them (Ingestion) or read from them (API).
- **Key Output**: A thread-safe `InMemoryCache` and `RedisCache` implementing the `Cache` trait.

### 4. Ingestion (`prd-ingestion.md`)
**Why fourth?** We need to populate the cache with data. This layer consumes from Kafka and writes to the Core Cache.
- **Key Output**: A background Tokio task consuming messages, resolving schemas, and writing to the configured stores.

### 5. API Layer (`prd-api.md`)
**Why last?** The API exposes the data stored by the Ingestion layer. It relies on the Configuration for routing, the Cache for data, and Observability for metrics.
- **Key Output**: An `Axum` web server serving JSON data from the cache.

---

## File Index
- [Configuration](./prd-configuration.md)
- [Observability](./prd-observability.md)
- [Core Cache](./prd-cache-core.md)
- [Ingestion](./prd-ingestion.md)
- [API](./prd-api.md)
