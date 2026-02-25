# Agent Guidance

This repository contains two implementations of the `push-cache` service:
1.  **Reference Implementation**: `rust-container` (Rust).
2.  **Port Implementation**: `java-container` (Java/Spring Boot).

## Specification Synchronization

**CRITICAL RULE**: Any changes to the codebase MUST be accompanied by updates to the corresponding specification files in the `spec/` directories.

-   **Rust Specs**: `rust-container/spec/`
-   **Java Specs**: `java-container/spec/`

### When modifying code:
1.  **Identify the Component**: Determine which PRD covers the modified code (e.g., `prd-api.md`, `prd-ingestion.md`).
2.  **Update the PRD**: Reflect any changes to behavior, configuration, or API contracts in the Markdown file.
3.  **Check Comparison**: If the change introduces or resolves a divergence between the Rust and Java implementations, you **MUST** update `java-container/spec/comparison.md`.

### Goal
The goal is to keep the `spec/` directories accurate enough that an AI could recreate the current state of the codebase solely from these documents.
