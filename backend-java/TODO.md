# Backend Java Parity Plan

This document outlines the steps required to bring the `backend-java` service to feature parity with the Rust `backend` service.

## Context
The Rust backend supports a sharded cache architecture where different data types (defined by Avro schemas) are routed to different cache stores (InMemory or Redis). The Java backend currently only supports a single InMemory store and no routing logic.

## Goal
Implement multi-store support (Redis + InMemory), schema-based routing for Kafka consumers, and path-based routing for the REST API.

---

## TODO List

- [x] **1. Configuration Parity**
    - [x] Refactor `CacheConfig` to support a list of `stores` (definitions) and `routes` (path mappings).
    - [x] Create `StoreDefinition` class (supports `type` discriminator for `in_memory` vs `redis`).
    - [x] Create `RouteDefinition` class.
    - [x] Ensure configuration binding works with `snake_case` YAML keys (Rust compatibility).

- [x] **2. Cache Abstraction**
    - [x] Define a `Cache` interface (Methods: `put`, `get`, `remove`, `getName`, `clear`?).
    - [x] Refactor existing `CacheStore` to `InMemoryCache` implementing this interface.
    - [x] Ensure implementations use `CompletableFuture` or are thread-safe non-blocking compatible. (Using Synchronous for now, consistent with Spring MVC)

- [ ] **3. Store & Route Initialization**
    - [ ] Create a `CacheFactory` or service that initializes all configured stores at startup.
    - [ ] Build the `schema_to_store` map (Schema Name -> Store Name) from configuration.
    - [ ] Build the `path_to_store` map (URL Path -> Store Name) from configuration.

- [ ] **4. Kafka Consumer Routing**
    - [ ] Update `KafkaConsumerService` to fetch the schema ID and look up the schema name.
    - [ ] Implement logic to route the message to the specific `Cache` instance based on the schema name.
    - [ ] Handle Tombstones: Broadcast delete to *all* stores (matching Rust behavior).

- [ ] **5. REST API Routing**
    - [ ] Refactor `RecordHandler` (or introduce a `RouterService`) to determine the target store based on the request path (prefix).
    - [ ] Ensure dynamic nesting of routers is simulated (e.g., specific paths route to specific stores).

- [ ] **6. Redis Implementation**
    - [ ] Add `spring-boot-starter-data-redis` dependency.
    - [ ] Implement `RedisCache` class using `StringRedisTemplate` (or generic `RedisTemplate<String, byte[]>`).
    - [ ] Configure `Lettuce` connection factory based on the `RedisConfig` from YAML.

- [ ] **7. Startup Checks**
    - [ ] Update `StartupCheckRunner` to verify connectivity to *all* configured Redis stores.
    - [ ] Ensure `HamsService` health checks reflect the status of all backends.

- [ ] **8. Metrics Alignment**
    - [ ] Ensure Prometheus metrics (cache size, hits, misses) are tagged by `store_name` to match Rust's granularity.
    - [ ] Verify metric names match Rust (`requests_total`, `cache_size`, etc.).

---

## Original Prompt Guidelines
*   Review the code in the rust design (backend). Identify all the features and compare them with the code in the backend-java.
*   Identify what has changed in the rust system and outline a set of small steps that can be taken one at a time.
*   Do not execute all steps at once only do a small incremental step.
*   Write the full set of steps into a todo file inside the backend-java directory and then update it as you progress.
