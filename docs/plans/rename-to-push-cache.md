---
description: Plan for renaming the tool and charts to push-cache
---

# Implementation Plan: Rename to push-cache

## Task 1: Update Makefile and README
1. Update `IMAGE_NAME` in `Makefile`.
2. Update project title and descriptions in `README.md`.

## Task 2: Backend Renaming
1. Update `name = "push-cache"` in `backend/Cargo.toml`.
2. Update `Dockerfile` to reflect the new package name and build artifacts.
3. Update `backend/test-data/config-localhost.yaml` and other test files.

## Task 3: Chart Renaming
1. Rename `charts/service-capture-backend` directory to `charts/push-cache-backend`.
2. Rename `charts/service-capture-backend-sample.yaml` to `charts/push-cache-backend-sample.yaml`.
3. Update `Chart.yaml` in the backend chart.
4. Search and replace `service-capture` with `push-cache` in all chart templates and values.

## Task 4: Infrastructure Files (FluxCD)
1. Search and replace `service-capture` with `push-cache` in `fluxcd-dev/` directory.

## Task 5: Verification
1. Run `make backend-test`.
2. Run `make backend-docker` to verify image building.
3. Final commit.
