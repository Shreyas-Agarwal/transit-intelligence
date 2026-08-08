# ADR 0003: Event-Driven Architecture & Phase 2 Transition

## Status

Approved (Partially superseded by [ADR 0008](file:///D:/transit-intelligence/docs/adr/008-redpanda-as-immutable-snapshot-ledger.md), which accelerates the deployment of Redpanda into Phase 1 to serve as the platform's immutable temporal snapshot ledger)

## Context

High-throughput telemetry ingestion (such as GPS coordinates from transit fleets) requires non-blocking ingestion and robust queue staging before storage. Initially in Phase 1, we require minimal operational overhead for local execution, but must prepare for high-scalability message ingestion.

## Decision

- **Phase 1:** Use **Redis Streams** as the lightweight message broker for local event processing and queuing.
- **Phase 2:** Shift the ingestion queue to **Redpanda** (a lightweight, modern, C++ based Kafka-compatible engine).
- All event contracts will be defined in a unified format inside the `packages/event-contracts` or `packages/shared-types` directories to make the transition transparent to worker logic.

## Consequences

- **Pros:**
  - Fast, low-overhead setup in Phase 1.
  - Linear horizontal scaling in Phase 2 via Kafka partition models.
  - Zero changes to consumer interface structure due to strict event schemas.
- **Cons:**
  - Need to support dual brokers in development configurations or plan a complete cutover.
  - Slightly higher setup complexity for Redpanda docker containers.
