# PRD: Core Cache Layer (Java)

## 1. Introduction
The Core Cache Layer provides a consistent interface for storing data, whether in memory or in Redis. It mirrors the Rust `Cache` trait, ensuring feature parity for multi-store routing and thread-safe operations.

## 2. Goals
- Provide a `Cache` interface for abstracting storage operations.
- Implement a thread-safe `InMemoryCache`.
- Implement a `RedisCache` using Spring Data Redis.
- Support store configuration via `StoreDefinition`.
- Support key namespacing for Redis.

## 3. User Stories

### US-001: Abstract Cache Interface
**Description:** As a developer, I want to swap cache implementations without changing business logic.

**Acceptance Criteria:**
- [ ] Define `Cache` interface.
- [ ] Methods: `get(key)`, `put(key, value)`, `remove(key)`, `getKeys()`, `containsKey(key)`, `clear()`, `checkHealth()`.
- [ ] All methods block (synchronous API) to match Spring MVC model.

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

## 4. Functional Requirements

### Cache Interface
1.  **Contract**:
    ```java
    public interface Cache {
        String getName();
        void put(String key, byte[] value);
        byte[] get(String key);
        byte[] remove(String key);
        Set<String> getKeys();
        boolean containsKey(String key);
        void clear();
        void checkHealth() throws Exception;
    }
    ```

### InMemoryCache
2.  **Storage**: `ConcurrentHashMap`.
3.  **Metrics**: Uses `AtomicLong` for size, registered with `MetricsService`.

### RedisCache
4.  **Connection**: `Lettuce` (non-blocking driver).
5.  **Configuration**: From `StoreDefinition` (URI parsing for DB index `redis://host:port/dbIndex`).
6.  **Key Prefixing**: Transparently add/remove prefix on all operations.
7.  **Key Listing**: Iterative `SCAN` using `RedisTemplate.scan()`.

## 5. Non-Goals
- TTL support per key (not in interface).
- Async API (interface is synchronous).

## 6. Technical Considerations
- **Resource Management**: `RedisCache` implements `AutoCloseable` (and `CacheFactory` is `DisposableBean`) to ensure connections are closed on shutdown.
- **Serialization**: Keys are Strings, Values are byte arrays (`RedisSerializer.byteArray()`). No object serialization happens here.

## 7. Success Metrics
- `InMemoryCache` handles high concurrency without errors.
- `RedisCache` correctly handles database selection (e.g., DB 1 vs DB 0).
- `getKeys` on Redis does not block the main thread for long periods.
