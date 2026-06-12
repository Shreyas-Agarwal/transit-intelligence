# Sprint 01 Specification

- **Period:** Week 24 (June 01, 2026 – June 12, 2026)
- **Theme:** Backend Modernization & Architectural Baseline Synchronization

## Objectives

Establish a modern, low-overhead backend runtime foundation and sync the project documentation and roadmap structure with recent strategic decisions (Redpanda acceleration).

## Weekly Activities & Tasks

### 1. Backend Framework Migration
- [x] Refactor the Core API (`apps/api`) to replace Express with Fastify.
- [x] Set up strict schema-driven request/response handling.
- [x] Configure Fastify plugins and structured logging boundaries.
- [x] Verify compilation using `pnpm run build` and ensure test suites pass.

### 2. Architecture & Decision Log Synchronization
- [x] Author ADR 0008: Adopt Redpanda as Immutable Temporal Snapshot Ledger.
- [x] Author ADR 0009: Standardize Backend Runtime on Fastify Instead of Express or NestJS.
- [x] Update `DECISIONS.md` to reference the correct ADR 0008 and 0009 entries.
- [x] Clean up conflicting references (Express vs. Fastify, Redis Streams vs. Redpanda) in:
  - `README.md`
  - `ARCHITECTURE.md`
  - `docs/design/system-context.md`
  - `docs/adr/0003-event-driven-architecture-and-phase2-transition.md`

### 3. Roadmap & Sprint Tracking Restructuring
- [x] Delete the outdated root `ROADMAP.md`.
- [x] Standardize on `docs/roadmap/!readme.md` as the high-level roadmap source of truth.
- [x] Set up the weekly sprint tracking structure under `docs/roadmap/phase-1/`.

### 4. Engineering Governance Baseline
- [x] Author CONTRIBUTING.md
- [x] Define branch strategy and release workflow
- [x] Define commit message conventions
- [x] Define Definition of Done checklist
- [x] Establish code review expectations for AI-generated code

### 5. Architecture Visualization
- [x] Create C4 System Context diagram
- [x] Create Container diagram
- [x] Create Data Flow diagram for GTFS-RT ingestion pipeline
- [x] Create Event Flow diagram showing Redpanda integration

### 6. Developer Environment Validation
- [x] Verify fresh-clone setup on clean environment
- [x] Document local startup process
- [x] Validate Docker Compose workflows
- [x] Verify pnpm workspace commands