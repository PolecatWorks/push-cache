# PRD: Core Cache Layer

## 1. Introduction
The Core Cache Layer provides an abstraction over different storage backends, allowing the application to support both high-speed in-memory caching and persistent, shared caching via Redis. This layer defines the contract that the ingestion and API layers interact with.

## 2. Goals
- Provide a unified `Cache` trait for all storage backends.
- Implement a thread-safe, high-concurrency in-memory store.
- Implement a non-blocking Redis store.
- Implement a non-blocking MongoDB store.
- Implement a non-blocking Postgres store.
- Ensure all operations are asynchronous.
- Support key namespacing (prefixes) for Redis.

## 3. User Stories

### US-001: Abstract Cache Interface
**Description:** As a developer, I want to interact with the cache using a standard interface so that the underlying storage can be swapped without changing business logic.

**Acceptance Criteria:**
- [ ] Define async `Cache` trait.
- [ ] Methods: `get(key)`, `insert(key, value)`, `remove(key)`, `keys()`, `contains_key(key)`.
- [ ] Implementations must be `Send + Sync`.

### US-002: In-Memory Implementation
**Description:** As an operator, I want to use local memory for lowest latency when persistence isn't required.

**Acceptance Criteria:**
- [ ] Use `DashMap` for concurrent access without global locking.
- [ ] Update `cache_size` Prometheus metric on insert/remove.
- [ ] `keys()` returns a snapshot of all keys.

### US-003: Redis Implementation
**Description:** As an operator, I want to use Redis for shared state across multiple instances.

**Acceptance Criteria:**
- [ ] Use `redis-rs` async connection manager.
- [ ] Support optional key `prefix` configuration (e.g., `myapp:key`).
- [ ] `insert` uses `SET`.
- [ ] `remove` uses `GETDEL` (returns the old value while deleting).
- [ ] `keys` uses `SCAN` iterator to avoid blocking the Redis server (do not use `KEYS *`).
- [ ] `get` uses `GET`.

### US-004: MongoDB Implementation
**Description:** As an operator, I want to use MongoDB as a persistent document store cache.

**Acceptance Criteria:**
- [ ] Use `mongodb` crate with tokio runtime.
- [ ] Support `url`, `database`, and `collection` configurations.
- [ ] Documents stored in the format `{ "key": <String>, "value": <Binary> }`.
- [ ] `insert` performs an upsert.
- [ ] `remove` returns the old value via `find_one_and_delete`.

### US-005: Postgres Implementation
**Description:** As an operator, I want to use Postgres as a persistent relational store cache.

**Acceptance Criteria:**
- [ ] Use `sqlx` crate with tokio runtime and Postgres driver.
- [ ] Support `url`, `table_name`, and optional `pool_size` configurations.
- [ ] Rows stored in a table with a `key` (Text, Primary Key) and `value` (Bytea) column.
- [ ] `insert` performs an upsert via `ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value`.
- [ ] `remove` returns the old value via `DELETE ... RETURNING value`.

## 4. Functional Requirements

### Cache Trait
1.  **Interface Definition**:
    ```rust
    #[async_trait]
    pub trait Cache: Send + Sync {
        async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, MyError>;
        async fn insert(&self, key: String, value: Vec<u8>) -> Result<(), MyError>;
        async fn remove(&self, key: &str) -> Result<Option<Vec<u8>>, MyError>;
        async fn keys(&self) -> Result<Vec<String>, MyError>;
        async fn contains_key(&self, key: &str) -> Result<bool, MyError>;
    }
    ```

### InMemoryCache
2.  **Storage**: `DashMap<String, Vec<u8>>`.
3.  **Metrics**: Takes a `Box<IntGauge>` in constructor for size tracking.

### RedisCache
4.  **Connection**: Use `redis::aio::ConnectionManager` for auto-reconnection and multiplexing.
5.  **Key Formatting**: If `prefix` is set, all keys sent to Redis are `prefix:key`. Keys returned from `keys()` must have the prefix stripped.
6.  **Error Handling**: Map Redis errors to `MyError`.

### MongoCache
7.  **Connection**: Use `mongodb::Client` directed at a specific database and collection.
8.  **Storage Format**: A BSON document containing `key` (String) and `value` (Bson Binary) is stored. The byte array is stored as a generic binary subtype.
9.  **Error Handling**: Map MongoDB errors to `MyError`.

### PostgresCache
10. **Connection**: Use `sqlx::PgPool` connected to the specified database URL. The connection pool size defaults to 5.
11. **Storage Format**: Data is stored in rows within the specified `table_name`, mapping the `key` string to the `value` byte array (`BYTEA` column).
12. **Error Handling**: Map SQLx errors to `MyError`. Note: the table is NOT created automatically; it must be provisioned externally.

## 5. Non-Goals
- TTL (Time To Live) support per key. (Currently not implemented in the trait).
- Eviction policies (LRU/LFU) for In-Memory cache (DashMap grows indefinitely until memory exhaustion, relying on OS/Container limits).

## 6. Technical Considerations
- **Concurrency**: `DashMap` is chosen over `RwLock<HashMap>` to reduce contention.
- **Redis Performance**: `SCAN` is critical for `keys()` to prevent "stop-the-world" pauses on large Redis instances.
- **Data Type**: All values are stored as raw `Vec<u8>` (bytes), preserving the exact payload received from Kafka.

## 7. Success Metrics
- In-Memory cache handles concurrent reads/writes without deadlocks.
- Redis cache recovers connection automatically after Redis restart.
- `keys()` operation on Redis does not cause latency spikes for other clients.
