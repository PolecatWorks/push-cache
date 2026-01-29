# Push Cache

[![Helm CI](https://github.com/PolecatWorks/push-cache/actions/workflows/helm-publish.yaml/badge.svg)](https://github.com/PolecatWorks/push-cache/actions/workflows/helm-publish.yaml)

[![backend Docker](https://github.com/PolecatWorks/push-cache/actions/workflows/backend-docker-publish.yml/badge.svg)](https://github.com/PolecatWorks/push-cache/actions/workflows/backend-docker-publish.yml)

**Push Cache** is a high-performance, in-memory caching service written in Rust. It consumes customer data from a Kafka topic (Avro formatted) and exposes it via a fast HTTP API. It is designed to be a sidecar or microservice that provides low-latency access to eventually consistent data.

## Architecture

The service consists of two main components running concurrently:
1.  **Kafka Consumer**: Ingests `Customer` updates from a Kafka topic, deserializes Avro messages, and updates the in-memory cache. It handles "tombstone" records (null payload) by removing entries.
2.  **Web Service**: An Axum-based HTTP server that serves the cached data to clients.

```mermaid
graph TD
    K[Kafka Topic] -- Avro Messages --> C(Kafka Consumer)
    SR[Schema Registry] -- Schema Validation --> C
    C -- Insert/Update/Remove --> M[(In-Memory Cache)]

    Client[HTTP Client] -- GET /api/users/:id --> API(Web Service)
    API -- Lookup --> M
    M -- Customer Data --> API
    API -- JSON Response --> Client
```

## Data Structures

### Customer Model
The core data entity is the `Customer`.

| Field       | Type   | Description |
|-------------|--------|-------------|
| `accountId` | String | Unique identifier (Key) |
| `name`      | String | Customer Name |
| `address`   | String | Customer Address |
| `phone`     | String | Contact Phone |
| `createdAt` | i64    | Creation timestamp |
| `updatedAt` | i64    | Last update timestamp |

## API Reference

### Get Customer
Retrieves a customer by their Account ID.

- **URL**: `/api/users/{account_id}`
- **Method**: `GET`
- **Response**: `200 OK` (JSON) or `404 Not Found`
- **Headers**:
    - `Cache-Control`: public, max-age={config.seconds}
    - `ETag`: "{updatedAt}"

### Create Customer
Manually adds a new customer to the cache.

- **URL**: `/api/users`
- **Method**: `POST`
- **Body**: JSON object matching the `Customer` model.
- **Response**:
    - `201 Created`: Returns the created customer.
    - `409 Conflict`: If the user already exists.

### Delete Customer
Manually removes a customer from the cache.

- **URL**: `/api/users/{account_id}`
- **Method**: `DELETE`
- **Response**:
    - `200 OK`: Returns the deleted customer.
    - `404 Not Found`: If the user does not exist.

### List Customers (Keys)
Lists all customer keys (account IDs) in the cache. Supports pagination and filtering.

- **URL**: `/api/users`
- **Method**: `GET`
- **Query Parameters**:
    - `limit` (optional): Number of keys to return (default: all).
    - `offset` (optional): Number of keys to skip (default: 0).
    - `filter` (optional): Filter keys by substring.
- **Response**: `200 OK` with a JSON array of strings (keys).

## Configuration

Configuration is handled via `figment` and can be supplied via a YAML file or environment variables (`APP_`).

| Section | Key | Default | Description |
|---------|-----|---------|-------------|
| **webservice** | `address` | `0.0.0.0:8080` | Bind address for the API |
| | `prefix` | `/api` | API path prefix |
| **kafka** | `brokers` | *Required* | Kafka bootstrap servers |
| | `group_id` | *Required* | Consumer group ID |
| | `topic` | *Required* | Topic name to consume |
| | `schema_registry_url` | *Required* | URL for Schema Registry |
| | `cache_max_age_seconds` | `300` | HTTP Cache-Control max-age |

Example `config.yaml`:
```yaml
webservice:
  address: "0.0.0.0:8080"
  prefix: "/api"

kafka:
  brokers: "localhost:9092"
  group_id: "push-cache-group"
  topic: "users"
  schema_registry_url: "http://localhost:8081"
  cache_max_age_seconds: 60
```

## Development

### Prerequisites
- Rust (latest stable)
- Kafka & Zookeeper (local or remote)
- Schema Registry
- `make`

### Quick Start
1. **Start dependencies** (in separate terminals or background):
    ```bash
    make start-zookeeper
    make start-kafka
    make start-schema
    ```

2. **Run the backend**:
    ```bash
    make backend-dev
    ```

### Testing
Run unit tests and doctests:
```bash
make backend-test
# OR directly:
cd backend && cargo test
```

### Docker Build
The project uses `cargo-chef` for optimized Docker layer caching.
```bash
make backend-docker
```

## Operations

- **Metrics**: Prometheus metrics are processed via `axum-prometheus`.
- **Health Checks**: Integrated via `libhams`.
- **Logging**: Structured logging via `tracing` and `tracing-subscriber`. Log level controlled via `CAPTURE_LOG` (default: WARN).

## External Libraries
- `libhams`: Custom library for service health and management. (Linking handled automatically in `build.rs`).

## Benchmarks

Single query performance of `DashState` (DashMap) with growing state sizes.

| State Size | Time (ns) | Trend |
| :--- | :--- | :--- |
| 100 | ~40.5 | Baseline |
| 1,000 | ~37.9 | Fast |
| 10,000 | ~56.1 | +48% |
| 100,000 | ~128.9 | +130% |
| 1,000,000 | ~267.1 | +107% |
| 5,000,000 | ~322.0 | +20% |
| **16,000,000** | **~350-400** | **Projected** |

To run benchmarks:
```bash
cargo bench --bench cache_benchmark
```

## Concurrent Benchmarks

Throughput with 1,000,000 entries and varying concurrency (Tokio tasks).

| Concurrency | Throughput (QPS) |
| :--- | :--- |
| 1 | ~52 K |
| 10 | ~290 K |
| 50 | ~450 K |
| 100 | ~1.2 M |
| 500 | ~1.5 M |
| **1000** | **~1.9 M** |

To run:
```bash
cargo bench --bench concurrent_benchmark
```

### Impact of State Size on Concurrency

Throughput with 100 concurrent tasks and varying state sizes.

| State Size | Throughput (QPS) |
| :--- | :--- |
| 100,000 | ~1.16 M |
| 1,000,000 | ~1.55 M |
| 5,000,000 | (Incomplete) |

### Insertion Performance

| Benchmark | Result | Notes |
| :--- | :--- | :--- |
| Single Insert Latency | ~167 ns | Overwrite existing key |
| Concurrent Insert Throughput | ~1.5 M QPS | 100 concurrent tasks |

To run insertions:
```bash
cargo bench --bench cache_benchmark -- insert_performance
cargo bench --bench concurrent_benchmark -- concurrent_insert_performance
```
