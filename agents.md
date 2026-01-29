# Agents

## push-cache
**Description:** A high-performance, in-memory caching service written in Rust. It consumes customer data from a Kafka topic (Avro formatted) and exposes it via a fast HTTP API.
**Type:** Service / Kafka Consumer
**Location:** /backend

## Rules
- **Explicit Configuration:** do not add defaults to configuration structures.
- **Chart Default Configuration:** Default application configs for helm charts get added to the config.yaml file and the values.yaml file is only for overriding that at deployment.
