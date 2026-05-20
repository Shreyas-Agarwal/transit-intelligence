# Project Roadmap

This roadmap details the planned stages of development for the **Transit Intelligence Platform**.

## Phase 1 — Foundation (Current)

Goal: Bootstrapping infrastructure, configurations, and core structures.

- [x] Configure monorepo workspace (`pnpm`, `turbo.json`, root `package.json`).
- [x] Establish linting, formatting, and compiler configs.
- [x] Initialize Git structures, pull request templates, and issue tracking.
- [x] Create core documentation overlays, governance, and first 5 ADRs.
- [ ] Implement initial docker compose stack (Postgres + Redis base).
- [ ] Build shared configuration, types, logger, and error packages.

## Phase 2 — Core Platform & Scalability Transition

Goal: Build out the ingestion pipeline, session validation, and database analytics storage.

- [ ] Setup authentication & authorization primitives.
- [ ] Create core REST API gateway proxying requests.
- [ ] **Data Pipeline Scalability:**
  - Spin up **Redpanda** (Kafka-compatible) to handle concurrent GPS message streams.
  - Deploy **ClickHouse** as the target analytical store for telemetry pings.
- [ ] Implement basic telemetry ingestion worker streaming data into Kafka, batching it into ClickHouse.

## Phase 3 — Domain Engines & Portal Interface

Goal: Implement business features, maps, real-time routing, and management dashboard.

- [ ] **Fleet Management Module:**
  - Vehicle listing, driver registration, and live tracking UI on maps.
- [ ] **Geofencing & Alerts:**
  - Real-time geofence checks (inside worker processes) emitting event alerts.
- [ ] **Reporting Engine:**
  - Build analytical query reports using ClickHouse aggregations (speed, delay, route adherence).

## Phase 4 — Operational Maturity

Goal: Hardening the system, load tests, and incident response planning.

- [ ] Implement tracing (OpenTelemetry) and structured metric scraping (Prometheus).
- [ ] Configure incident management runbooks (recovery playbooks).
- [ ] Conduct load and chaos testing (high-throughput vehicle simulation).
