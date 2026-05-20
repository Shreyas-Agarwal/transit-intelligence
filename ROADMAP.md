# Project Roadmap

This roadmap details the planned stages of development for the **Transit Intelligence Platform**.

## Phase 1 — Foundation (Current)

Goal: Bootstrapping infrastructure, configurations, core packages, and establishing Swiss GTFS-RT ingestion with DuckDB analytics.

- [x] Configure monorepo workspace (`pnpm`, `turbo.json`, root `package.json`).
- [x] Establish linting, formatting, and compiler configs.
- [x] Initialize Git structures, pull request templates, and issue tracking.
- [x] Create core documentation overlays, governance, and first 5 ADRs.
- [x] Implement initial docker compose stack (Postgres + Redis base).
- [x] Build shared configuration, types, logger, and error packages.
- [ ] **Swiss Open Data Ingestion:**
  - Build ingestion worker polling Zurich/Swiss GTFS Static and GTFS-RT protobuf feeds at 30s intervals.
- [ ] **DuckDB Analytics Bridge:**
  - Deploy **DuckDB** as the embedded, file-backed column-store database for Phase 1 analytics.
  - Implement a dynamic pathfinding calculator based on _temporally variable weighted graph theory_ (using dynamic travel times from live GTFS-RT delays).

## Phase 2 — Core Platform & Scalability Transition

Goal: Scale out the telemetry pipeline, transition to big data warehousing, and establish messaging brokers.

- [ ] Setup authentication & authorization primitives.
- [ ] Create core REST API gateway proxying requests.
- [ ] **Data Pipeline Scalability:**
  - Transition the dynamic graph calculations and telemetry log archiving from DuckDB to **ClickHouse** for high-volume storage.
  - Deploy **Redpanda (Kafka-compatible)** messaging brokers to decouple concurrent ingestion feeds from execution processors.
- [ ] Implement robust event stream consumer workers writing high-throughput telemetry updates.

## Phase 3 — Domain Engines & Portal Interface

Goal: Implement business features, maps, real-time routing, and management dashboard.

- [ ] **Fleet Management Module:**
  - Vehicle listing, driver registration, and live tracking UI on maps.
- [ ] **Geofencing & Alerts:**
  - Real-time geofence checks (inside worker processes) emitting event alerts.
- [ ] **Reporting Engine:**
  - Build analytical query reports using ClickHouse/DuckDB aggregations (speed, delay, route adherence).

## Phase 4 — Operational Maturity

Goal: Hardening the system, load tests, and incident response planning.

- [ ] Implement tracing (OpenTelemetry) and structured metric scraping (Prometheus).
- [ ] Configure incident management runbooks (recovery playbooks).
- [ ] Conduct load and chaos testing (high-throughput vehicle simulation).
