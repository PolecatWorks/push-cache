# PRD: API Layer

## 1. Introduction
The API Layer exposes the cached data via a RESTful HTTP interface. It features dynamic routing, allowing different URL paths to map to different underlying cache stores (sharding). It also handles the conversion of stored Avro binary data into JSON for client consumption.

## 2. Goals
- Provide a read-heavy API for retrieving cached records.
- Support dynamic routing based on configuration (e.g., `/users` -> Redis, `/settings` -> Memory).
- Handle content negotiation (Store Avro -> Serve JSON).
- Provide administrative endpoints for manual cache manipulation (Create/Delete).
- Integrate with the observability stack (metrics/logging).

## 3. User Stories

### US-001: List Keys
**Description:** As a client, I want to list available keys in a specific cache namespace.

**Acceptance Criteria:**
- [ ] `GET [base_path]/[route_path]` (e.g., `/api/users`).
- [ ] Support `limit` (int) and `offset` (int) query parameters for pagination.
- [ ] Support `filter` (string) query parameter for substring matching on keys.
- [ ] Return JSON array of string keys.
- [ ] Return 200 OK.

### US-002: Get Record
**Description:** As a client, I want to retrieve a record by its ID in JSON format.

**Acceptance Criteria:**
- [ ] `GET [base_path]/[route_path]/:id`.
- [ ] Look up raw bytes in the routed Store.
- [ ] If missing, return 404 Not Found.
- [ ] If found:
    - [ ] Validate Magic Byte (0).
    - [ ] Extract Schema ID.
    - [ ] Resolve Schema from cache.
    - [ ] Deserialize Avro to JSON.
    - [ ] Return 200 OK with JSON body.
    - [ ] Set `Cache-Control: public, max-age=[config.max_age]` header.

### US-003: Create Record (Manual)
**Description:** As a system, I want to manually insert raw Avro data into the cache (e.g., for testing or restoring).

**Acceptance Criteria:**
- [ ] `POST [base_path]/[route_path]/:id`.
- [ ] Body: Raw binary data (Confluent Wire Format).
- [ ] Validate Magic Byte (0) and Schema ID presence.
- [ ] Insert directly into the routed Store.
- [ ] Return 201 Created.

### US-004: Delete Record
**Description:** As a system, I want to manually remove a record from the cache.

**Acceptance Criteria:**
- [ ] `DELETE [base_path]/[route_path]/:id`.
- [ ] Remove key from the routed Store.
- [ ] If found, return 200 OK (with raw byte array as JSON list).
- [ ] If not found, return 404 Not Found.

## 4. Functional Requirements

### Routing Logic
1.  **Dynamic Nesting**: The web server must iterate over `config.cache.routes`. For each route:
    *   Find the configured Store instance.
    *   Create a dedicated `Router` with the standard endpoints (`/`, `/:id`).
    *   Nest this router under `[config.webservice.address.path] + [route.path]`.
    *   *Example*: If base is `/api` and route is `/users` -> Store A, then `/api/users/123` hits Store A.

### Data Conversion
2.  **Read Path**:
    *   Store (Vec<u8>) -> `apache_avro::from_avro_datum` -> `serde_json::Value` -> HTTP Response.
3.  **Write Path**:
    *   HTTP Request Body (Bytes) -> Store (Vec<u8>). No validation against schema registry happens here (assumed valid wire format).

### Error Handling
4.  **Standard Errors**:
    *   404: Key not found, or Schema not found.
    *   400: Bad Request (serialization errors).
    *   500: Internal Server Error (Redis down, IO errors).
5.  **Format**: JSON body `{ "message": "error description" }`.

## 5. Non-Goals
- Content-Type negotiation for returning raw Avro (currently always returns JSON).
- Authentication/Authorization (handled by upstream gateway/mesh).

## 6. Technical Considerations
- **Axum State**: Use `RouteState` to inject the specific `Arc<dyn Cache>` into the handlers for each route, avoiding global lookups per request.
- **Performance**: Deserialization is CPU intensive. It is done on the web server thread pool (Tokio).

## 7. Success Metrics
- p99 Latency for `GET` requests < 50ms (for in-memory).
- Correctly routing requests to different backends based on path.
