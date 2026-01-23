---
description: Rename the tool and charts to push-cache
---

# Specification: Rename to push-cache

## 1. Overview
The current project is a scaffold from `service-capture`. We need to rename all occurrences of `service-capture` to `push-cache` to reflect its new purpose.

## 2. Changes Required

### 2.1 Backend (Rust)
- **Cargo.toml**: Update `name` to `push-cache`.
- **Dockerfile**: Update references and paths if any.
- **Source Code**: Update strings or logs that mention "Service Capture".

### 2.2 Helm Charts
- Rename directory `charts/service-capture-backend` to `charts/push-cache-backend`.
- **Chart.yaml**: Update `name` and `description`.
- **values.yaml**: Update any default values or labels.
- **Templates**: Ensure `_helpers.yaml` and other templates use the new name.
- Rename `charts/service-capture-backend-sample.yaml` to `charts/push-cache-backend-sample.yaml`.

### 2.3 Configuration & CI/CD
- **Makefile**: Update `IMAGE_NAME` and any other references.
- **fluxcd-dev/**: Update YAML files referencing the old name.
- **test-data/**: Update sample configs and secrets.
- **README.md**: Update titles and descriptions.

## 3. Success Criteria
- `make backend-test` passes.
- `make backend-docker` passes (builds `push-cache-backend` image).
- No occurrences of `service-capture` remain in the codebase (except for history/old configs if explicitly desired).
