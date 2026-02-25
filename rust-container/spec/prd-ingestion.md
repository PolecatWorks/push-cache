# PRD: Data Ingestion (Kafka)

## 1. Introduction
The Ingestion layer is responsible for consuming messages from a Kafka topic, resolving their Avro schemas, and routing the data to the appropriate cache store based on the schema type. It also handles "tombstones" (delete markers) to keep the cache consistent with the source of truth.

## 2. Goals
- Consume high-volume Kafka streams efficiently.
- Support Confluent Wire Format (Magic Byte + Schema ID).
- Dynamically fetch and cache Avro schemas from Schema Registry.
- Route messages to specific cache stores (sharding) based on Schema Fullname.
- Handle Tombstones (null payloads) as global deletes.
- Track consumer lag and expose it for readiness checks.

## 3. User Stories

### US-001: Consume Avro Messages
**Description:** As a system, I must consume messages encoded with Avro from a configured topic.

**Acceptance Criteria:**
- [ ] Connect to Kafka using `rdkafka`.
- [ ] Join consumer group specified in config.
- [ ] Validate message structure (Magic Byte 0 + 4-byte Schema ID).
- [ ] Log warning and increment `schema_mismatch_count` for invalid messages.

### US-002: Schema Resolution
**Description:** As a system, I must know the schema of a message to route it correctly.

**Acceptance Criteria:**
- [ ] Extract Schema ID from payload bytes 1-4.
- [ ] Check local `schema_cache` (DashMap).
- [ ] If missing, fetch from Schema Registry (`GET /schemas/ids/{id}`).
- [ ] Parse and cache the schema.
- [ ] Handle failures (log error, skip message).

### US-003: Schema-Based Routing
**Description:** As a developer, I want specific data types to go to specific stores (e.g., "Users" to Redis, "Settings" to Memory).

**Acceptance Criteria:**
- [ ] Extract Fullname from the resolved Avro schema (e.g., `com.polecatworks.billing.Customer`).
- [ ] Look up target Store Name in configuration map.
- [ ] If found, insert `(Key, Full Payload)` into the target Store.
- [ ] If not found, increment `schema_unrouted_count` and log warning.

### US-004: Handle Tombstones
**Description:** As a system, I must remove data when a delete marker is received.

**Acceptance Criteria:**
- [ ] Detect messages with `null` payload.
- [ ] If Key is present, remove that Key from **ALL** configured stores.
- [ ] Increment `tombstones_processed`.

### US-005: Lag Tracking & Readiness
**Description:** As an operator, I want to know when the cache has caught up with the stream.

**Acceptance Criteria:**
- [ ] Enable `rdkafka` statistics callback (interval 1s).
- [ ] Sum consumer lag across all assigned partitions.
- [ ] Update `consumer_lag` metric.
- [ ] When lag hits 0, mark the application as "Ready" (via Hams probe).

## 4. Functional Requirements

### Consumer Configuration
1.  **Group ID**: Support explicit string OR hostname-based group ID (for unique broadcast-like consumption if needed, though typically explicit for shared cache).
2.  **Auto Commit**: Enabled (`true`).
3.  **Offset Reset**: Configurable (`earliest` or `latest`).
4.  **Force Reset**: Optional flag `force_reset_earliest` to manually seek to beginning on rebalance (useful for rebuilding cache).

### Message Processing Flow
5.  **Step 1**: Receive Message.
6.  **Step 2**: Check Payload.
    *   If `None` -> **Tombstone Path**: Iterate all stores, call `remove(key)`.
    *   If `Some` -> **Insert Path**:
        *   Validate Magic Byte.
        *   Extract ID.
        *   Resolve Schema (Cache -> Registry).
        *   Get Schema Fullname.
        *   Find Store.
        *   Call `store.insert(key, payload)`.

### Schema Registry Client
7.  **Implementation**: Use direct HTTP client (`reqwest`) to fetch `GET /schemas/ids/{id}` to avoid dependency bugs/limitations.
8.  **Caching**: Store parsed `apache_avro::Schema` objects in a `DashMap<u32, Schema>`.

## 5. Non-Goals
- Deserializing the full record body in the consumer. The consumer stores the **raw bytes** directly to save CPU. Deserialization happens only on read (API).
- Producing messages back to Kafka.

## 6. Technical Considerations
- **Concurrency**: The consumer runs in a dedicated Tokio task.
- **Error Handling**: Database/Store errors during insert should be logged but typically shouldn't crash the consumer (unless critical).
- **Blocking**: Store operations are `async`, ensuring the consumer doesn't block the thread.

## 7. Success Metrics
- Consumer keeps up with producer rate (lag remains low).
- `schema_unrouted_count` remains 0 (configuration matches data).
- Cache populates correctly after a fresh start.
