# Agent Guidelines - Push Cache

## Project Overview

**push-cache** is a high-performance, in-memory caching service written in Rust. It consumes customer data from a Kafka topic (Avro formatted) and exposes it via a fast HTTP API.

**Type:** Service / Kafka Consumer
**Location:** `/backend`

---

## 1. Project-Specific Rules

### Configuration Standards
*   **Explicit Configuration**: Do not add defaults to configuration structures. All configuration options must be explicit.
*   **Chart Default Configuration**: Default application configs for Helm charts get added to the `config.yaml` file. The `values.yaml` file is only for overriding configuration at deployment time.

---

## 2. Feature Development Process

When designing new features, follow this sequence:

1.  **Interview**: Ask questions to clarify ambiguities and understand the core requirements.
2.  **Specification**: Write a technical spec in `docs/specs/feature-name.md` and ask for review.
3.  **Implementation Plan**: Once the spec is approved, create a plan in `docs/plans/feature-name.md` and ask for review.
4.  **Execution**: Proceed with development only after the plan is approved.

---

## 3. Coding and Testing Standards

*   **Test-Driven Development**: Always ensure relevant tests are implemented alongside features.
*   **Verification**: Before committing, ensure all tests pass and the project builds successfully.
*   **Command**: Use `make backend-test` (or `cargo test` in the `backend` directory) to verify backend changes.

---

## 4. Commit and Batch Completion

*   **Micro-Commits**: Make a commit once a task is completed and tests are green.
*   **Batch Completion**: When a batch of tasks completing a feature is done, ensure the Docker build passes.
*   **Command**: Use `make backend-docker` to verify the build.
*   **Commit Pattern**: Use Conventional Commits (e.g., `feat:`, `fix:`, `chore:`, `test:`).

---

## 5. Environment and Dependencies

*   The backend requires Kafka and a schema repository for integration tests.
*   Refer to the `Makefile` for local environment setup (e.g., `make start-kafka`, `make start-schema`).
