# PRD: Configuration System (Java)

## 1. Introduction
The Java configuration system mirrors the Rust implementation, leveraging Spring Boot's powerful configuration loading capabilities to support YAML files, environment variables, and directory-based secrets. It wraps the Spring Boot startup with a CLI to provide a compatible interface for operators.

## 2. Goals
- Achieve feature parity with the Rust CLI (`start`, `version`, `config-check`).
- Support loading configuration from an external YAML file.
- Support loading secrets from a directory (Kubernetes style) using Spring `configtree`.
- Validate configuration strongly using Bean Validation (JSR-380).
- Map deeply nested configuration structures (sharded cache) to Java POJOs.

## 3. User Stories

### US-001: CLI Interface
**Description:** As an operator, I want to use the same command-line arguments as the Rust application to manage the Java service.

**Acceptance Criteria:**
- [ ] Implement `picocli` for argument parsing.
- [ ] `start -c <FILE> -s <DIR>`: Starts the application with the given config and secrets.
- [ ] `config-check -c <FILE> -s <DIR>`: Validates config without starting the full service.
- [ ] `version`: Prints version information.

### US-002: Configuration Loading
**Description:** As a system, I want to merge configuration from multiple sources with specific precedence.

**Acceptance Criteria:**
- [ ] Source 1: YAML file provided via `--config`.
- [ ] Source 2: Secrets directory provided via `--secrets` (mapped as `spring.config.import=optional:configtree:...`).
- [ ] Source 3: Environment variables (Spring Boot standard binding).
- [ ] Bind these sources to the `AppConfig` class.

### US-003: Strong Validation
**Description:** As a developer, I want the application to fail fast if configuration is missing or invalid.

**Acceptance Criteria:**
- [ ] Use `jakarta.validation` annotations (`@NotNull`, `@NotBlank`, `@Min`).
- [ ] Validate nested objects (cascading validation with `@Valid`).
- [ ] `config-check` command must trigger this validation and exit with error if it fails.

## 4. Functional Requirements

### CLI Implementation
1.  **Library**: `picocli`.
2.  **Start Command**:
    *   Sets `spring.config.additional-location` to the value of `--config`.
    *   Sets `spring.config.import` to `optional:configtree:{secrets_dir}/`.
    *   Launches `SpringApplication`.
3.  **Config Check Command**:
    *   Sets same config properties.
    *   Sets `spring.main.web-application-type=none`.
    *   Starts and immediately closes the application context to verify bean creation and config binding.

### Configuration Structure (`AppConfig`)
4.  **Binding**: Use `@ConfigurationProperties` (no prefix, binding to root).
5.  **Components**:
    *   `hams`: `HamsConfig`.
    *   `runtime`: `RuntimeConfig`.
    *   `webservice`: `WebServiceConfig` (`address` as `URI`).
    *   `kafka`: `KafkaConfig` (`brokers` as `URI`).
    *   `startupChecks`: `StartupCheckConfig`.
    *   `cache`: `CacheConfig`.

### Cache Configuration
6.  **Stores**: List of `StoreDefinition`.
    *   `type`: Enum (`IN_MEMORY`, `REDIS`).
    *   `name`: String.
    *   `schemas`: List of strings.
    *   `url`: URI (Redis only).
    *   `prefix`: String (Redis only).
7.  **Routes**: List of `RouteDefinition`.
    *   `path`: String.
    *   `store`: String.
    *   `key_from_body`: String (optional, e.g. "userId").

## 5. Non-Goals
- Hot reloading of configuration.

## 6. Technical Considerations
- **ConfigTree**: Spring Boot 2.4+ `configtree:` volume mounting support matches the Rust `figment` file adapter behavior (filename = key).
- **Snake Case**: Spring Boot automatically binds `snake-case` properties (YAML) to `camelCase` Java fields.
- **URI handling**: Java's `java.net.URI` is used for `address` and `brokers`, which matches Rust's `url::Url` parsing logic reasonably well.

## 7. Success Metrics
- Pass `config-check` with a valid `config.yaml`.
- Fail `config-check` with an invalid `config.yaml`.
- `start` command correctly reads secrets from a directory (e.g., database password).
