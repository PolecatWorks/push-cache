# PRD: API Layer (Java)

## 1. Introduction
The API Layer exposes the cached data via RESTful endpoints, dynamically creating routes based on configuration. It handles content negotiation by converting stored Avro binary data into JSON responses using the Apache Avro library.

## 2. Goals
- Provide dynamic routing using Spring WebMvc Functional Endpoints (`RouterFunctions`).
- Create nested routers for each configured path -> store mapping.
- Deserialize Avro to JSON on read.
- Provide administrative write/delete endpoints.
- Integrate with Micrometer metrics.

## 3. User Stories

### US-001: List Keys
**Description:** As a client, I want to see keys in a cache.

**Acceptance Criteria:**
- [ ] `GET [base_path]/[route_path]`.
- [ ] Query params: `limit`, `offset`, `filter`.
- [ ] Return JSON array of strings.
- [ ] Filter by substring match.
- [ ] Sort alphabetically.

### US-002: Get Record
**Description:** As a client, I want to get a record as JSON.

**Acceptance Criteria:**
- [ ] `GET [base_path]/[route_path]/:id`.
- [ ] Increment `requests_total`.
- [ ] If missing, increment `requests_miss`, return 404.
- [ ] If found:
    - [ ] Validate Magic Byte.
    - [ ] Extract Schema ID.
    - [ ] Get Schema from `SchemaService`.
    - [ ] Use `GenericDatumReader` and `JsonEncoder` to convert to JSON.
    - [ ] Return 200 OK.
    - [ ] Set `Cache-Control: public, max-age=[config.max_age]`.

### US-003: Create Record
**Description:** As a system, I want to insert raw data manually.

**Acceptance Criteria:**
- [ ] `POST [base_path]/[route_path]/:id`.
- [ ] Body: `byte[]`.
- [ ] Validate Magic Byte.
- [ ] `cache.put(id, body)`.
- [ ] Return 201 Created.

### US-004: Delete Record
**Description:** As a system, I want to delete data manually.

**Acceptance Criteria:**
- [ ] `DELETE [base_path]/[route_path]/:id`.
- [ ] `cache.remove(id)`.
- [ ] Return 200 OK (with removed value if present).

## 4. Functional Requirements

### Routing Logic (`WebConfig`)
1.  **Dynamic Setup**: Iterate `appConfig.getCache().getRoutes()`.
2.  **Handler Creation**: Instantiate a new `RecordHandler` for the target store.
3.  **Router Function**: Use `RouterFunctions.route().path(...)` to mount handlers.

### Data Conversion (`RecordHandler`)
4.  **Serialization**: Use `apache-avro` library (`GenericDatumWriter`, `JsonEncoder`) to serialize the `GenericRecord` to JSON bytes.

### Error Handling
5.  **Exceptions**: Catch exceptions (e.g., deserialization errors) and return 500 Internal Server Error with JSON `{ "message": "..." }`.

## 5. Non-Goals
- Annotation-based controllers (`@RestController`). The functional routing style is preferred for dynamic path generation.

## 6. Technical Considerations
- **Concurrency**: `RecordHandler` is thread-safe (stateless except for injected services).
- **Performance**: Deserialization happens on the servlet thread.

## 7. Success Metrics
- Requests are routed to the correct store based on the URL path.
- JSON output matches the Avro schema structure.
