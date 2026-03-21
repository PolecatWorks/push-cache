# PRD: Core Cache Layer (Java)

## 1. Introduction
The Core Cache Layer provides a consistent interface for storing data, whether in memory or in Redis. It mirrors the Rust `Cache` trait, ensuring feature parity for multi-store routing and thread-safe operations.

## 2. Goals
- Provide a `Cache` interface for abstracting storage operations.
- Implement a thread-safe `InMemoryCache`.
- Implement a `RedisCache` using Spring Data Redis Reactive (`ReactiveRedisTemplate`).
- Implement a `MongoCache` matching the Rust implementation's BSON document structure.
- Implement an `OracleCache` using standard JDBC matching the Rust application's BLOB/VARCHAR2 schema.
- Implement a `PostgresCache` matching the Rust application's BYTEA/VARCHAR schema.
- Support store configuration via `StoreDefinition`.
- Support key namespacing for Redis.

## 3. User Stories

### US-001: Abstract Cache Interface
**Description:** As a developer, I want to swap cache implementations without changing business logic.

**Acceptance Criteria:**
- [ ] Define `Cache` interface.
- [ ] Methods: `get(key)`, `put(key, value)`, `remove(key)`, `getKeys()`, `containsKey(key)`, `clear()`, `checkHealth()`.
- [ ] Return Reactor types (`Mono`, `Flux`) to provide non-blocking operations.

### US-002: In-Memory Implementation
**Description:** As an operator, I want a fast, local cache for transient data.

**Acceptance Criteria:**
- [ ] Use `ConcurrentHashMap<String, byte[]>`.
- [ ] Update `cache_size` Micrometer gauge on put/remove.
- [ ] Implement `clear()` (unlike Rust, which doesn't expose it).
- [ ] `checkHealth()` always returns healthy.

### US-003: Redis Implementation
**Description:** As an operator, I want a persistent, shared cache for distributed deployments.

**Acceptance Criteria:**
- [ ] Use `LettuceConnectionFactory`.
- [ ] Use `RedisTemplate<String, byte[]>`.
- [ ] Parse Redis URL (host, port, password, database index).
- [ ] Support `prefix` configuration. keys stored as `prefix:key`, returned without prefix.
- [ ] `getKeys()` uses `SCAN` (via `ScanOptions`) to avoid blocking.
- [ ] `checkHealth()` performs a `PING`.
- [ ] Implement `AutoCloseable` to clean up connections.

### US-004: Mongo Implementation
**Description:** As an operator, I want a robust document database backend for data persistence and advanced querying capabilities in the future.

**Acceptance Criteria:**
- [ ] Use standard MongoDB Java Driver (`mongodb-driver-sync` / `spring-boot-starter-data-mongodb`).
- [ ] Handle BSON `Binary` formats natively to replicate the document structure of the Rust application (`{"key": "my_key", "value": <binary>}`).
- [ ] `checkHealth()` performs a `ping` against the `admin` database.
- [ ] Implement `AutoCloseable` to clean up `MongoClient` connections.

### US-005: Oracle Implementation
**Description:** As an operator, I want to use an Oracle database as a persistent cache backend to match Rust capabilities.

**Acceptance Criteria:**
- [ ] Use `ojdbc11` and Spring's `JdbcTemplate` with a `HikariDataSource`.
- [ ] Support `url` and `tableName` configurations.
- [ ] Keys are stored as `VARCHAR2(255)` (Primary Key) and values as `BLOB`.
- [ ] `insert` performs an upsert using `MERGE INTO`.
- [ ] Add CLI subcommand `create-schemas` to automatically create the configured Oracle tables before the service runs using JDBC.

### US-006: Postgres Implementation
**Description:** As an operator, I want to use a PostgreSQL database as a persistent cache backend to match Rust capabilities.

**Acceptance Criteria:**
- [x] Use `postgresql` JDBC driver and Spring's `JdbcTemplate` with a `HikariDataSource`.
- [x] Support `url` and `tableName` configurations.
- [x] Keys are stored as `VARCHAR` (Primary Key) and values as `BYTEA`.
- [x] `insert` performs an upsert using `ON CONFLICT DO UPDATE`.
- [x] Update CLI subcommand `create-schemas` to automatically create the configured Postgres tables before the service runs using JDBC.

## 4. Functional Requirements

### Cache Interface
1.  **Contract**:
    ```java
    import reactor.core.publisher.Flux;
    import reactor.core.publisher.Mono;

    public interface Cache {
        String getName();
        Mono<Void> put(String key, byte[] value);
        Mono<byte[]> get(String key);
        Mono<byte[]> remove(String key);
        Flux<String> getKeys();
        Mono<Boolean> containsKey(String key);
        Mono<Void> clear();
        Mono<Void> checkHealth();
    }
    ```

### InMemoryCache
2.  **Storage**: `ConcurrentHashMap`.
3.  **Metrics**: Uses `AtomicLong` for size, registered with `MetricsService`.

### RedisCache
4.  **Connection**: `Lettuce` (non-blocking driver).
5.  **Configuration**: From `StoreDefinition` (URI parsing for DB index `redis://host:port/dbIndex`).
6.  **Key Prefixing**: Transparently add/remove prefix on all operations.
7.  **Key Listing**: Iterative `SCAN` using `ReactiveRedisTemplate.scan()`.

### MongoCache
8.  **Connection**: `MongoClient` from `com.mongodb.client.MongoClients`.
9.  **Format**: Stores items in the target collection identically to the Rust equivalent (`"key"` is String, `"value"` is `org.bson.types.Binary`).
10. **Listing**: Standard cursor-based document enumeration.

### OracleCache
11. **Connection**: `HikariDataSource` with `JdbcTemplate`.
12. **Format**: Data is stored in rows with `k` (VARCHAR2) and `v` (BLOB).
13. **Sync**: All Oracle interactions run synchronously, matching the application's overall web server model.
14. **Management**: The `create-schemas` CLI command handles automatic setup.

### PostgresCache
15. **Connection**: `HikariDataSource` with `JdbcTemplate`.
16. **Format**: Data is stored in rows with `k` (VARCHAR) and `v` (BYTEA).
17. **Management**: The `create-schemas` CLI command handles automatic setup.

## 5. Non-Goals
- TTL support per key (not in interface).
- Full end-to-end reactive ingestion loop (KafkaConsumer polling is still synchronous and uses `.block()` appropriately).

## 6. Technical Considerations
- **Resource Management**: `RedisCache` implements `AutoCloseable` (and `CacheFactory` is `DisposableBean`) to ensure connections are closed on shutdown.
- **Serialization**: Keys are Strings, Values are byte arrays (`RedisSerializer.byteArray()`). No object serialization happens here.

## 7. Success Metrics
- `InMemoryCache` handles high concurrency without errors.
- `RedisCache` correctly handles database selection (e.g., DB 1 vs DB 0).
- `getKeys` on Redis does not block the main thread for long periods.
