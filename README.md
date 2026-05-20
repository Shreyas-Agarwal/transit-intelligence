# Transit Intelligence Platform

A high-performance, production-grade, event-driven transit network observability and operational analytics system built using a modular monorepo architecture. This platform models a transit network as a temporal graph driven by immutable event streams, utilizing live Swiss public transportation data.

---

## Surface-Level Explanation (For Non-Technical People)

### What Is This Project?

This project is a **real-time transit intelligence and observability platform** built using live Swiss public transportation data. Instead of simply showing routes, ETAs, and maps, the platform tries to understand **how the transit network behaves as a living system**.

### The Core Idea

Public transport systems are highly interconnected. A delay in one tram or train does not remain isolated. It can:

- Affect intersections
- Impact downstream schedules
- Increase waiting times
- Disrupt passenger transfers
- Propagate across the network

This platform studies those effects in real time.

### What Problem Are We Trying To Solve?

Most transit applications focus on navigation, journey planning, and ticketing. Very few focus on **operational intelligence**, meaning:

- Network reliability
- Delay propagation
- Congestion behavior
- Route stability
- System recovery

The project aims to make the transit network observable the way modern software systems are observable.

### Simple Example

Imagine one tram in Zurich gets delayed by 4 minutes. That small delay may:

1. Cause platform crowding
2. Affect traffic signal priorities
3. Delay following trams
4. Disrupt passenger transfers
5. Create ripple effects elsewhere

The project tries to **detect, visualize, measure, and eventually predict** those ripple effects.

### What Will The Platform Show?

- **Real-time Operational Map:** Live vehicle movement, delayed routes, and network hotspots.
- **Reliability Metrics:** Which routes are unstable, which stations create bottlenecks, and which parts of the network recover slowly.
- **Delay Propagation Visualization:** Not just "tram delayed," but "this delay is now affecting 4 connected segments."
- **Historical Replay:** Replay the network state during rush hours, weather disruptions, large public events, or accidents.

### Why Is This Interesting?

Because transit systems behave similarly to distributed software systems, network infrastructure, power grids, and other complex systems where small failures create cascading effects. This project combines:

- Software engineering
- Data engineering
- Real-time systems
- Graph analysis
- Operational analytics

### Why Switzerland?

Switzerland exposes extremely high-quality public transit data. Zurich is especially interesting because the network is dense, highly synchronized, operationally efficient, and sensitive to small disruptions, creating rich system behavior worth studying t is also a city I spent meaningful time in, which created a personal interest in understanding how such a tightly coordinated transit system operates beneath the surface.

### What Makes This Different From A Typical Portfolio Project?

This is not another dashboard, CRUD app, or AI wrapper. It is designed as a **real-time operational intelligence system**. The focus is on event-driven architecture, temporal state modeling, network analytics, system observability, and scalable data pipelines.

---

## Deep Technical Definition (Engineering Perspective)

### System Definition

The platform is an **event-driven transit network observability and operational analytics system** built on real-time GTFS/GTFS-RT transit feeds. It models the transit network as a **temporal graph**, driven by **immutable event streams**, from which operational state is continuously derived.

### Core Architectural Philosophy: Events Are Source Of Truth

Every operational change becomes an immutable event (e.g. vehicle position update, delay update, trip cancellation, abnormal dwell time). State is never treated as primary truth. Instead:

$$\text{Events} \longrightarrow \text{Derived State}$$

This enables replayability, historical reconstruction, temporal analytics, and future prediction models.

### Domain Model

- **Nodes:** Represent stations, intersections, and transfer hubs.
- **Edges:** Represent route segments, travel corridors, and directional transit paths.
- **Events:** Represent arrivals, departures, delays, schedule deviations, and congestion indicators.

### Key Technical Concepts

1. **Temporal Graph Modeling:** The transit network is modeled as a time-dependent weighted graph. Edge weights dynamically change based on delays, congestion, operational instability, and traversal variance.
2. **Operational State Derivation:** Real-time state is continuously computed from event streams to derive metrics like route health, congestion pressure, network fragility, recovery efficiency, and propagation amplification.
3. **Event Streaming Architecture:** Real-time GTFS feeds are ingested into **Redpanda** (or Apache Kafka later) for decoupled processing, replayability, temporal consistency, and consumption by multiple independent services.

### Internal Architectural Pattern: Modular Monolith

The system follows a **modular monolith architecture** with isolated domain modules, event-driven internals, protobuf contracts, and optional gRPC interfaces. This avoids premature microservice complexity while preserving modular boundaries, scalability paths, and clear domain ownership.

### Proposed Internal Modules

- `/ingestion` — Polls and processes incoming GTFS/GTFS-RT protobuf streams.
- `/transit_state` — Manages static timetables and current active vehicle positions.
- `/analytics` — Computes reliability metrics, recovery profiles, and historical logs.
- `/graph_engine` — Manages the temporal transit graph and runs pathfinding/propagation queries.
- `/prediction` — Runs anomaly detection and delay propagation forecasting.
- `/alerting` — Triggers alerts on service anomalies or threshold violations.
- `/api_gateway` — Single entry point for routing, authentication, and rate limiting.

### Communication Patterns

- **Asynchronous:** Event propagation (delay events, position updates, anomalies) is handled through Kafka/Redpanda topics.
- **Synchronous:** Operational queries (current route health, graph snapshots, path calculations) are handled through **gRPC** using protobuf contracts.

### Storage Architecture

- **Operational State:** PostgreSQL (for relational data like static GTFS schedules) and Redis (for fast, ephemeral cache states).
- **Analytical/Event Storage:** ClickHouse (in Phase 2) for append-heavy workloads, real-time OLAP, time-series analytics, and fast aggregation. DuckDB is utilized as the file-backed column store for Phase 1.

### Prediction Philosophy

Prediction is treated as a **derived intelligence layer**, NOT the core product. The project prioritizes reliable event modeling, state consistency, and operational semantics. Only after stable observability exists do we introduce anomaly detection, propagation forecasting, and network instability predictions.

### Long-Term Vision

The architecture is intentionally domain-generalizable. The same event-driven operational intelligence framework could later support airports, logistics systems, utilities, supply chains, and smart infrastructure networks. The real platform abstraction is: **real-time operational observability for interconnected physical systems**.

---

## Project Structure

```text
transit-intelligence/
├── apps/
│   ├── web/          # React/TypeScript/Vite Frontend Portal
│   ├── api/          # Express/TypeScript Core REST API
│   ├── workers/      # Ingestion, processing, and polling workers
│   ├── gateway/      # API Gateway (NGINX proxy initially)
│   └── cli/          # Command-Line administrative tooling
├── packages/
│   ├── shared-config/# ESLint, Prettier, and TypeScript base configurations
│   ├── shared-types/ # Shared domain data types and schemas
│   ├── shared-logger/# Structured logger with correlation and request tracing
│   ├── shared-errors/# Centralized standard error boundary codes and handler types
│   └── shared-db/    # Shared database connection drivers (Postgres, DuckDB, ClickHouse)
├── infrastructure/
│   ├── docker/       # Custom service Dockerfiles
│   ├── nginx/        # NGINX gateway configurations
│   └── monitoring/   # Prometheus, Grafana, and instrumentation dashboards
├── docs/
│   ├── architecture/ # Detailed system boundaries, contexts, data-flows
│   ├── adr/          # Architecture Decision Records (sequentially numbered)
│   └── design/       # Detailed component/lifecycle design documents
└── tests/            # Integration, E2E, load, and contract tests
```

## Quick Start

### Prerequisites

Ensure you have the following installed:

- Node.js `v24.15.0`
- pnpm `v11.1.3`
- Docker and Docker Compose

### Local Development

1. **Clone the repository and install dependencies:**

   ```bash
   pnpm install
   ```

2. **Spin up local infrastructure (Postgres, Redis):**

   ```bash
   docker-compose up -d
   ```

3. **Build shared packages:**

   ```bash
   pnpm run build
   ```

4. **Run the development servers:**
   ```bash
   pnpm run dev
   ```

## Architectural Governance

Every major decision is documented in sequential Architecture Decision Records in `docs/adr/`. Refer to [ARCHITECTURE.md](file:///D:/transit-intelligence/ARCHITECTURE.md) for a block diagram and communications breakdown, and [AGENTS.md](file:///D:/transit-intelligence/AGENTS.md) for guidelines governing human-agent coordination.
