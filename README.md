# Transit Intelligence

Real-time observability and operational analytics for public transportation networks.

> **Pre-1.0.** Core services are under active development and are not yet integrated end to end. The repository is public to support open discussion of the architecture and design — see [Development Status](#development-status) below before attempting to run it.

Transit Intelligence models a transit network as a temporal graph driven by immutable event streams, exposing how delays, congestion, and instability propagate across the network as they happen.

The platform targets live Swiss public transportation data, with Zürich as its primary deployment target. Today, ingestion runs against static GTFS datasets; live GTFS-RT integration is still ahead — see [Development Status](#development-status).

---

## Why Transit Intelligence

Most transit software is built for passengers: journey planning, ETAs, ticketing. Very little software is built for the network itself.

Public transit systems are tightly coupled. A single delayed tram doesn't stay isolated — it propagates: platforms crowd, signal priorities shift, following vehicles slip, transfers break, and the disruption spreads outward through the network. Operators and engineers studying this behavior have had no equivalent of the observability tooling that exists for distributed software systems — no event timeline, no replay, no propagation graph, no reliability metrics.

Transit Intelligence treats a transit network the way modern infrastructure treats a distributed system: as a set of observable, replayable services. It answers questions standard transit apps don't ask — which routes are unstable, which stations bottleneck the network, how far a single delay travels before it dissipates, and how quickly the system recovers.

---

## Philosophy

Transit Intelligence treats operational state as a derived consequence of immutable events rather than mutable application state. This architecture enables deterministic replay, historical reconstruction, and consistent operational analytics across evolving transit networks.

---

## Core Concepts

Transit Intelligence models a transit network as a distributed operational system, built around a small set of concepts:

- Vehicles emit immutable operational events.
- Events produce derived operational state.
- State updates modify a temporal transit graph.
- Analytics operate on graph evolution rather than raw telemetry.
- Historical replay reconstructs prior network conditions from event history.

---

## Platform Capabilities

**Real-Time Operational Map** — Live vehicle positions, active delays, and network hotspots as they occur.

**Delay Propagation Visualization** — Traces how an individual delay event affects connected segments, transfers, and downstream schedules, rather than reporting it as an isolated incident.

**Reliability Metrics** — Continuously computed route health, congestion pressure, network fragility, and recovery efficiency.

**Historical Replay** — Reconstructs and replays past network states, including rush hours, weather disruptions, and major public events.

**Temporal Graph Engine** — Represents the network as a time-dependent weighted graph, with edge weights driven by live delay, congestion, and traversal variance.

---

## System Architecture

Transit Intelligence is built as a modular monolith: isolated domain modules, event-driven internals, and protobuf-defined service contracts. Each module owns a clear boundary, and all cross-module communication runs through events or typed RPC contracts — never shared mutable state.

This separation is what makes the system replayable: any historical state can be reconstructed by replaying the event log that produced it.

### Modules

| Module | Responsibility |
|---|---|
| `ingestion` | Consumes and normalizes GTFS / GTFS-RT protobuf streams |
| `transit_state` | Maintains static timetables and live vehicle positions |
| `graph_engine` | Maintains the temporal transit graph; serves pathfinding and propagation queries |
| `analytics` | Computes reliability, recovery, and historical metrics |
| `network_intelligence` | Anomaly detection and delay propagation forecasting |
| `alerting` | Triggers alerts on anomalies and threshold violations |
| `api_gateway` | Single entry point for routing, auth, and rate limiting |

### Communication

- **Asynchronous** — Delay events, position updates, and anomalies propagate through event streaming topics.
- **Synchronous** — Operational queries (route health, graph snapshots, path calculations) are served over gRPC using protobuf contracts.

A full architectural breakdown, including data flow diagrams and module boundaries, lives in [`docs/architecture/`](docs/architecture/).

---

## Technology Stack

| Layer | Technology |
|---|---|
| Event Streaming | Redpanda |
| Service Communication | gRPC / Protobuf |
| Operational Store | PostgreSQL |
| Cache | Redis |
| Analytical Store | DuckDB |
| OLAP Engine | ClickHouse |
| API | Fastify / TypeScript |
| Frontend | React / TypeScript / Vite |
| Observability | Prometheus, Grafana |

Technology selection rationale is documented separately in [`docs/architecture/technology-rationale.md`](docs/architecture/technology-rationale.md).

---

## Deployment Targets

Transit Intelligence is designed for:

- Public transit operators
- Smart city platforms
- Mobility researchers
- Urban planning organizations
- Transportation control centers

---

## Project Structure

Per ADR 0013 (domain-first workspace organization), the repository is organized around bounded contexts, not languages — each domain owns its own manifests and is independently buildable/testable/lintable from within its own directory.

```text
transit-intelligence/
├── domains/
│   ├── ingestion/      # Rust — GTFS-S/GTFS-RT acquisition (ckan, realtime, service-alerts)
│   └── gtfs_s/         # Python — GTFS static subset pipeline (uv-managed)
├── infrastructure/
│   ├── docker/         # Service Dockerfiles
│   ├── nginx/          # Gateway configuration
│   └── monitoring/     # Prometheus, Grafana, dashboards
├── docs/
│   ├── architecture/   # System boundaries, contexts, data flows
│   ├── adr/            # Architecture Decision Records
│   ├── design/         # Component and lifecycle design documents
│   └── development/    # Per-service setup and run instructions
└── mise.toml           # Repository-wide toolchain version defaults
```

An earlier TypeScript-based prototype (`apps/*`, shared `packages/*`) explored a frontend, REST API, and gateway; it didn't go anywhere operationally and has been removed. `network-explorer` now lives as its own separate project outside this repository.

---

## Development Status

Transit Intelligence is pre-1.0. The sections above describe the target architecture; the table below reflects what is actually implemented today.

| Component | Status |
|---|---|
| Ingestion service (GTFS / GTFS-RT) | Working |
| Static dataset analysis | Complete |
| Static dataset → temporal graph conversion | In progress |
| Ideal-conditions simulation | Not started |
| Live data integration onto the graph | Not started |
| End-to-end local orchestration (`docker-compose up`) | Not yet functional |

Phase 1 is focused on building the temporal graph from the static dataset, validating it under simulated ideal conditions, and only then layering live GTFS-RT data on top. Until that sequence is complete, there is no single command that brings up a working end-to-end environment.

This repository is public to support discussion and review of the architecture rather than to invite local deployment at this stage. The `ingestion` service can be run and inspected independently.

---

## Documentation

- [Architecture Overview](docs/architecture/) — system boundaries, module contracts, and data flow
- [Technology Rationale](docs/architecture/technology-rationale.md) — role and selection criteria for each platform component
- [Architecture Decision Records](docs/adr/) — sequential record of architectural decisions
- [Development Guide](docs/development/) — running and inspecting individual services in their current state
- [ARCHITECTURE.md](ARCHITECTURE.md) — block diagram and communication breakdown
- [AGENTS.md](AGENTS.md) — guidelines for human-agent coordination in this repository

### Related Engineering Projects

This platform is supported by two independent engineering research repositories:

- **go-event-lab** — software architecture and distributed systems
- **Lakehouse Engineering Lab** — analytical platforms and modern data engineering

These repositories evaluate architectural alternatives that inform, but are intentionally decoupled from, Transit Intelligence itself.

---

## Future Platform Capabilities

- **Multimodal network expansion** — intercity rail, regional rail, buses, and ferry routes alongside existing tram coverage
- **Passenger pressure modeling** — inferred transfer pressure, station saturation, and recovery elasticity layered onto the existing operational graph
- **External system integration** — airport schedules and large-event calendars as upstream pressure sources for downstream congestion
- **Cross-network portability** — architecture validation against additional transit topologies beyond the initial deployment
- **Propagation and resilience research** — cascading failure analysis, fragility scoring, and disruption simulation built on top of the observability layer

---

## Contributing

Contributions are welcome. Please review the architectural documentation in `docs/architecture/` and open ADRs in `docs/adr/` before proposing structural changes. See `AGENTS.md` for repository conventions.
