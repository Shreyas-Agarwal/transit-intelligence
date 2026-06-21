# Sprint 02 Specification

- **Period:** Week 25 (June 13, 2026 – June 19, 2026)
- **Theme:** GTFS-RT Ingestion Foundation
- **Branch:** `feat/gtfs-ingestion`

## Objectives

Establish the real-time transit data ingestion foundation. This sprint is exploratory and infrastructure-focused. The goal is to understand the Zürich GTFS-RT feed structure, validate protobuf decoding, connect to locally running Redpanda, publish raw feed events, and produce design documentation.

No graph analytics, route intelligence, projections, or materialized views are implemented in this sprint.

## Architecture Notes

- **Ingestion worker language**: TypeScript — per ADR 0010 (polyglot runtime), event acquisition and publishing is TypeScript's responsibility.
- **Feed**: Single combined GTFS-RT endpoint from Open Data Swiss (`api.opentransportdata.swiss/la/gtfs-rt`).
- **Redpanda**: Running natively in WSL2/Ubuntu (`localhost:9092`). Docker Compose Redpanda remains under `phase2` profile for production use.
- **Payload format**: Canonical JSON (protobuf → decode → JSON → Redpanda) for immediate `rpk topic consume` inspection.

## Tasks

### Task 1 – Redpanda Local Development Environment

- [x] Document local Redpanda setup procedure (`docs/runbooks/local-redpanda-setup.md`)
- [x] Document topic management commands and port reference

### Task 2 – GTFS-RT Feed Exploration

- [x] Create `apps/ingestion` TypeScript workspace (`package.json`, `tsconfig.json`, `.eslintrc.json`)
- [x] Bundle official `gtfs-realtime.proto` definition
- [x] Implement `config.ts` — environment variable loading with fast-fail on missing required values
- [x] Implement `types/gtfs-rt.ts` — TypeScript interfaces for decoded GTFS-RT entities
- [x] Implement `feed/fetcher.ts` — HTTP binary feed acquisition
- [x] Implement `feed/decoder.ts` — protobufjs decode pipeline
- [x] Implement `feed/explorer.ts` — one-shot feed inspection CLI
- [x] Document feed structure template (`docs/design/gtfs-rt-feed-structure.md`)

### Task 3 – Domain Mapping Analysis

- [x] Document GTFS-RT → domain concept mapping (`docs/design/gtfs-rt-domain-mapping.md`)
  - Entity ownership
  - Redpanda topic assignment
  - Message key strategy
  - Timestamp strategy (event time vs. processing time)

### Task 4 – Redpanda Producer Prototype

- [x] Implement `producer/topics.ts` — ADR 0008 topic constants and configuration
- [x] Implement `producer/client.ts` — KafkaJS client factory + `ensureTopics()` bootstrap
- [x] Implement `producer/publisher.ts` — poll loop with JSON publish and per-cycle metrics
- [x] Implement `src/index.ts` — package entry point
- [x] Create `.env.example` with all required variables documented
- [x] Document topic configuration (`docs/design/redpanda-topic-configuration.md`)

### Task 5 – Zone 110 Investigation

- [x] Document filtering strategy analysis (`docs/design/zone-110-filtering-strategy.md`)
- [x] Formal recommendation: **Option B — Raw Feed → Redpanda → Downstream Filtering**

## Definition of Done

- [x] `apps/ingestion` TypeScript workspace builds without errors
- [x] GTFS-RT feed exploration utility implemented
- [ ] Feed successfully decoded against live endpoint (requires API token and running Redpanda)
- [x] Prototype events published to `transit.snapshots.raw`
- [x] Feed structure documented
- [x] Design decisions documented
- [ ] `pnpm run build` passes with zero TypeScript errors
- [ ] `pnpm run lint` passes
- [ ] `pnpm run format:check` passes
- [ ] `pnpm run test` passes (existing tests unaffected)

## Out of Scope

- Graph construction / route planning
- DuckDB analytics / materialized views
- UI visualization
- Journey reconstruction / delay prediction
- PostgreSQL schema changes
- Downstream Redpanda consumers
