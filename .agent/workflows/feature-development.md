---
description: Process for designing and implementing new features
---

This workflow ensures that features are well-defined, planned, and tested before deployment.

### 1. Interview & Discovery
Before writing any code or specs, interview the user to clarify:
- Core objectives and success criteria.
- Integration points with existing systems (Kafka, Schema Registry, etc.).
- Performance and scalability requirements.
- Edge cases and error handling.

### 2. Technical Specification
// turbo
1. Create a specification file in `docs/specs/<feature-name>.md`.
2. Document the architecture, API changes, data models, and dependencies.
3. Ask the user for a review of the spec.

### 3. Implementation Plan
// turbo
1. Create an implementation plan in `docs/plans/<feature-name>.md` once the spec is approved.
2. Break down the work into manageable tasks.
3. Define the testing strategy (unit, integration, and Docker verification).
4. Ask the user for a review of the plan.

### 4. Development & Verification
For each task in the plan:
1. Implement the feature and its tests.
2. Run `make backend-test` to ensure quality.
3. Commit with a descriptive message (e.g., `feat: added kafka consumer for bills`).
4. Once the feature is complete, run `make backend-docker` to verify the build.
