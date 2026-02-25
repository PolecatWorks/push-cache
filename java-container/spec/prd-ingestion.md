# PRD: Data Ingestion (Java)

## 1. Introduction
The Ingestion layer consumes Avro messages from Kafka, resolves schemas dynamically, and routes data to the correct cache store. It mirrors the Rust consumer logic, including manual lag tracking and direct Schema Registry interaction.

## 2. Goals
- Consume messages using the Java Kafka Client.
- Parse Confluent Wire Format (Magic Byte + Schema ID).
- Fetch schemas from Schema Registry via HTTP (bypassing Confluent SerDes for raw access).
- Route messages to specific stores based on Schema Fullname.
- Handle Tombstones as broadcast deletes.
- Track consumer lag manually and expose it via metrics.

## 3. User Stories

### US-001: Consume Avro Messages
**Description:** As a system, I must process a stream of Avro messages reliably.

**Acceptance Criteria:**
- [ ] Use `KafkaConsumer<String, byte[]>`.
- [ ] Subscribe to configured topic.
- [ ] Poll loop with 100ms timeout.
- [ ] Validate Magic Byte (0).
- [ ] Log warning/increment metric on schema mismatch.

### US-002: Schema Resolution
**Description:** As a system, I need the schema to determine the routing key.

**Acceptance Criteria:**
- [ ] Extract Schema ID (bytes 1-4).
- [ ] Check local cache (`ConcurrentHashMap`).
- [ ] If missing, HTTP GET `/schemas/ids/{id}` from Registry.
- [ ] Parse JSON response -> Extract schema string -> Parse Avro Schema object.
- [ ] Cache the parsed schema.

### US-003: Schema-Based Routing
**Description:** As a developer, I want to shard data based on its type.

**Acceptance Criteria:**
- [ ] Get Schema Fullname (e.g., `com.example.User`).
- [ ] Lookup Store in `CacheFactory`.
- [ ] `store.put(key, full_payload)`.
- [ ] If no store found, increment `schema_unrouted_count`.

### US-004: Handle Tombstones
**Description:** As a system, I must process deletes.

**Acceptance Criteria:**
- [ ] If `record.value() == null`:
- [ ] Iterate ALL stores in `CacheFactory`.
- [ ] `store.remove(key)`.
- [ ] Increment `tombstones_processed`.

### US-005: Lag Tracking
**Description:** As an operator, I want to see consumer lag.

**Acceptance Criteria:**
- [ ] Every 1000ms, calculate lag.
- [ ] `endOffsets()` - `position()` for all assigned partitions.
- [ ] Sum lag and update `MetricsService`.
- [ ] Update `LagClearedHealthIndicator` when lag hits 0.

## 4. Functional Requirements

### Kafka Consumer Service
1.  **Implementation**: Manual `Thread` running a `while` loop with `KafkaConsumer`.
2.  **Configuration**:
    *   `enable.auto.commit=true`.
    *   `auto.offset.reset` from config.
    *   `group.id` from config.
3.  **Rebalancing**: Log assignments. If `forceResetEarliest` is true, seek to beginning on assignment.

### Schema Service
4.  **Client**: `RestClient` (Spring) for HTTP calls.
5.  **Parsing**: `ObjectMapper` (Jackson) to parse Registry response, `Schema.Parser` (Avro) to parse schema string.
6.  **Concurrency**: `synchronized` fetch to prevent thundering herd on cache miss.

## 5. Non-Goals
- Using `@KafkaListener` (Spring Kafka) annotations. The manual control over the poll loop and lag calculation is preferred to match the Rust implementation structure.

## 6. Technical Considerations
- **Threading**: The consumer runs in a dedicated thread managed by `KafkaConsumerService`.
- **Deserialization**: Only the Schema ID is extracted. The payload is stored as raw bytes.

## 7. Success Metrics
- Consumer lag metric updates every second.
- Schemas are cached after the first lookup.
- Tombstones clear data from all stores.
