# PRD: Configuration System

## 1. Introduction
The configuration system provides a flexible, layered approach to configuring the `push-cache` service. It supports loading configuration from YAML files, merging secrets from a directory, and overriding values via environment variables. It also includes a CLI for managing the application lifecycle (start, version, config check).

## 2. Goals
- Load configuration from a primary YAML file.
- Merge secret values from a dedicated secrets directory (useful for Kubernetes volume mounts).
- Allow environment variable overrides (prefixed with `APP_`).
- Provide CLI commands to validate configuration without starting the service.
- Support complex nested structures for sharded cache configuration.

## 3. User Stories

### US-001: Start Application with Config
**Description:** As an operator, I want to start the application pointing to a specific config file and secrets directory.

**Acceptance Criteria:**
- [ ] Application accepts `start` subcommand.
- [ ] Application accepts `--config <FILE>` argument.
- [ ] Application accepts `--secrets <DIR>` argument (defaults to `secrets`).
- [ ] Application loads YAML from file, merges secrets, and applies env vars.
- [ ] Application logs loaded config at DEBUG level.
- [ ] Application fails to start if config is invalid.

### US-002: Check Configuration
**Description:** As an operator, I want to validate the configuration file before deploying.

**Acceptance Criteria:**
- [ ] Application accepts `config-check` subcommand.
- [ ] Accepts same `--config` and `--secrets` arguments as start.
- [ ] Exits with success (0) if config is valid.
- [ ] Exits with error (non-zero) and prints error details if config is invalid.
- [ ] Does NOT start the web server or Kafka consumer.

### US-003: Environment Overrides
**Description:** As an operator, I want to override specific config values using environment variables.

**Acceptance Criteria:**
- [ ] Env vars prefixed with `APP_` override config values.
- [ ] Nested keys are separated by `__` (double underscore).
- [ ] Example: `APP_WEBSERVICE__ADDRESS` overrides `webservice.address`.

### US-004: Define Cache Stores and Routes
**Description:** As a developer, I want to define multiple cache stores and routing rules in the configuration.

**Acceptance Criteria:**
- [ ] Config supports a `cache` section.
- [ ] `cache.stores` list allows defining `in_memory`, `redis`, `mongo`, and `postgres` stores.
- [ ] `cache.routes` list allows mapping URL paths to store names, and optionally `key_from_body`.
- [ ] `redis` store config includes `url` and optional `prefix`.
- [ ] `mongo` store config includes `url`, `database`, and `collection`.
- [ ] `postgres` store config includes `url`, `table_name`, and optional `pool_size`.

## 4. Functional Requirements

### CLI
1.  **Framework**: Use `clap` for parsing command line arguments.
2.  **Commands**:
    *   `version`: Print application and library versions.
    *   `start`: Run the service. Args: `--config` (required), `--secrets` (optional).
    *   `config-check`: Validate config. Args: `--config` (required), `--secrets` (optional).

### Configuration Loading
3.  **Library**: Use `figment` for layered configuration.
4.  **Layers**:
    *   Layer 1: YAML content read from the file specified by `--config`.
    *   Layer 2: Secrets from the directory specified by `--secrets`. (Mapped using `FileAdapter`).
    *   Layer 3: Environment variables starting with `APP_`.

### Configuration Structure (`MyConfig`)
5.  **Hams**: `HamsConfig` (health/metrics).
6.  **Runtime**: `ThreadRuntime` (threading model).
7.  **Webservice**: `WebServiceConfig` (address, prefix).
8.  **Kafka**: `MyKafkaConfig` (brokers, topic, group_id, schema_registry, offset policies).
    *   `group_id`: Can be an explicit string OR `{ use_hostname: true }`.
    *   `preload_schemas`: List of integer Schema IDs to be fetched and cached at startup (optional). Startup will fail if any schema cannot be loaded.
9.  **Startup Checks**: `StartupCheckConfig` (timeout, fails, enabled).
10. **Cache**: `CacheConfig` (stores, routes).
    *   `stores`: List of definitions. Each has `name`, `type` (tagged, snake_case, supports `in_memory`, `redis`, `mongo`, `postgres`), `schemas`.
    *   `routes`: List of `path` -> `store` mappings.

## 5. Non-Goals
- Dynamic configuration reloading at runtime (requires restart).
- Support for formats other than YAML (e.g., JSON, TOML) for the main config file.

## 6. Technical Considerations
- **Secrets Handling**: `figment_file_provider_adapter` treats files in the secrets directory as keys. E.g., a file named `kafka.password` in the secrets dir containing "secret" maps to `kafka.password = "secret"`.
- **URL Handling**: Custom `UrlWithUsernamePassword` struct to handle credentials in URLs cleanly.

## 7. Success Metrics
- Configuration loads correctly in all deployed environments.
- Operators can diagnose config errors using `config-check` without application crash loops.
