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

- [x] **3. Store & Route Initialization**
    - [x] Create a `CacheFactory` or service that initializes all configured stores at startup.
    - [x] Build the `schema_to_store` map (Schema Name -> Store Name) from configuration.
    - [x] Build the `path_to_store` map (URL Path -> Store Name) from configuration.

- [x] **4. Kafka Consumer Routing**
    - [x] Update `KafkaConsumerService` to fetch the schema ID and look up the schema name.
    - [x] Implement logic to route the message to the specific `Cache` instance based on the schema name.
    - [x] Handle Tombstones: Broadcast delete to *all* stores (matching Rust behavior).

- [x] **5. REST API Routing**
    - [x] Refactor `RecordHandler` (or introduce a `RouterService`) to determine the target store based on the request path (prefix).
    - [x] Ensure dynamic nesting of routers is simulated (e.g., specific paths route to specific stores).

- [x] **6. Redis Implementation**
    - [x] Add `spring-boot-starter-data-redis` dependency.
    - [x] Implement `RedisCache` class using `StringRedisTemplate` (or generic `RedisTemplate<String, byte[]>`).
    - [x] Configure `Lettuce` connection factory based on the `RedisConfig` from YAML.

- [x] **7. Startup Checks**
    - [x] Update `StartupCheckRunner` to verify connectivity to *all* configured Redis stores.
    - [x] Ensure `HamsService` health checks reflect the status of all backends. (Implemented via `CacheHealthIndicator` in Actuator)

- [x] **8. Metrics Alignment**
    - [x] Ensure Prometheus metrics (cache size, hits, misses) are tagged by `store_name` to match Rust's granularity.
    - [x] Verify metric names match Rust (`requests_total`, `cache_size`, etc.).

---

## Original Prompt Guidelines
*   Review the code in the rust design (backend). Identify all the features and compare them with the code in the backend-java.
*   Identify what has changed in the rust system and outline a set of small steps that can be taken one at a time.
*   Do not execute all steps at once only do a small incremental step.
*   Write the full set of steps into a todo file inside the backend-java directory and then update it as you progress.

---

## Original Prompt
I a previous task I gave you the instructions: review the code in the rust design (backend). Identify all the features and compare them with the code in the backend-java.

Of the features identify what has changed in the rust system and outline a set of small steps that can be taken one at a time to update from the current state to having parity with the rust version. Do not execute all steps at once only do a small incremental step. Write the full set of steps into a todo file inside the backend-java directory and then update it as you progress. Also store this prompt in that file as a guideline for future iterations of progress.

Now I want you to pick up the todo and work on the next step
