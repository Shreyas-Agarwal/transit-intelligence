# ADR 0008: Adopt Redpanda as Immutable Temporal Snapshot Ledger

## Status

Approved

## Context

The transit observatory platform requires a reliable temporal backbone capable of storing and replaying operational state transitions across the Zürich transit network. While the system initially considered high-frequency real-time streaming semantics, Phase 1 architecture has shifted toward a discrete snapshot-driven observability model.

Under this model, the Swiss GTFS-RT feeds are polled every 20-30 seconds and treated as periodic operational “state freezes” of the transit network rather than as continuously mutating event streams.

Each snapshot represents the complete observed operational state of a region at a specific timestamp, including:

- Vehicle positions
- Delay updates
- Trip modifications
- Alerts and disruptions

The system’s analytical responsibility is therefore not to process micro-events continuously, but to compute and analyze the state transitions between consecutive temporal snapshots.

This architectural shift introduces several important requirements:

- Immutable historical replayability for operational reconstruction.
- Decoupling ingestion from downstream analytical consumers.
- Parallel consumption by multiple computational engines without contention.
- Deterministic temporal replay for future Monte Carlo simulations and resilience analysis.
- Preservation of chronological ordering for spatial-temporal trajectory reconstruction.

Although the platform does not require ultra-low-latency stream processing, it still benefits significantly from an append-only event ledger architecture capable of supporting replayable analytical pipelines.

## Decision

We adopt Redpanda as the platform’s immutable temporal event ledger and inter-service streaming backbone.

The following architectural decisions are established:

Snapshot-Based Temporal Ingestion

GTFS-RT polling workers will ingest transit data every 20-30 seconds and publish normalized snapshot payloads into Redpanda topics.

Each published message represents a coherent temporal observation window rather than a single low-level event mutation.

Redpanda as Immutable Replay Substrate

Redpanda will function primarily as:

- An append-only temporal ledger
- A replay substrate for operational reconstruction
- A decoupling mechanism between ingestion and analytics
- A shared source-of-truth for downstream computational consumers

The platform will treat historical snapshots as immutable observational records.

## Topic Topology

Phase 1 will intentionally maintain a minimal topic topology to avoid premature distributed-system complexity.

Initial topics include:

- "transit.snapshots.raw"
  - Raw normalized GTFS-RT snapshot payloads.

- "transit.snapshots.normalized"
  - Cleaned and structurally validated operational state snapshots.

- "transit.state.deltas"
  - Computed state transitions derived between consecutive snapshots.

- "transit.metrics.operational"
  - Derived observability and resilience metrics.

Additional domain-specific topics will not be introduced until justified by clear computational requirements.

## Consumer Separation

Independent analytical services will consume the same temporal snapshots in parallel, including:

- Delay propagation engines
- Temporal routing engines
- Replay systems
- Visualization materializers
- Future Monte Carlo simulation engines
- Future passenger pressure models

Consumers must remain stateless relative to one another and rely only on immutable stream history plus independently materialized state.

## Partitioning Strategy

Partitioning will prioritize preservation of spatial-temporal ordering semantics.

Messages associated with a single operational entity (such as trip_id or vehicle_id) should consistently resolve to the same partition wherever chronological trajectory reconstruction is required.

However, the architecture intentionally avoids over-specializing partitioning logic during Phase 1 until operational replay patterns are validated empirically.

Fastify Service Role

Fastify will not act as the primary computational ingestion engine.

Instead:

- Polling workers handle acquisition and normalization.
- Redpanda acts as the temporal transport and replay layer.
- Python/DuckDB workers handle computational analytics and graph processing.
- Fastify exposes orchestration APIs and materialized observability state to frontend consumers.

## Consequences

Pros

### Deterministic Replayability

Historical operational states can be replayed deterministically for debugging, visualization, simulation, and future research analysis.

### Strong Decoupling

Ingestion, analytics, visualization, and future ML systems remain operationally independent while consuming the same temporal truth source.

### Simulation Readiness

The immutable snapshot ledger creates a strong foundation for future Monte Carlo simulations and resilience experimentation.

### Simplified Temporal Reasoning

Snapshot-diff semantics significantly reduce the complexity associated with ultra-high-frequency event synchronization and race conditions.

### Multi-Consumer Scalability

Additional analytical engines can be introduced later without modifying ingestion infrastructure.

### Operational Observability Alignment

The architecture aligns with the platform’s observatory-oriented philosophy rather than attempting to emulate dispatch-grade real-time control systems.

## Cons

### Increased Infrastructure Complexity

Redpanda introduces operational overhead relative to simpler queue-based or direct polling architectures.

### Potential Overengineering Risk

Current throughput requirements are modest, meaning Redpanda is being adopted primarily for architectural semantics and replayability rather than raw scale requirements.

### Snapshot Granularity Tradeoff

20-30 second polling intervals may miss extremely short-lived transient operational changes.

### Partitioning Evolution Risk

Future analytical workloads may require repartitioning strategies as the observatory evolves toward pressure modelling and multimodal propagation analysis.
