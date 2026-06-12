# Architecture Diagrams & Visualizations

This document hosts the official architectural diagrams for the Transit Intelligence Platform, illustrating system boundaries, container responsibilities, data flow pipelines, and event-driven semantics.

---

## 1. C4 System Context Diagram

The C4 System Context diagram provides a high-level view of how users (Transit Operators) and external systems (Open Data Swiss) interact with the platform.

```mermaid
C4Context
    title System Context diagram for Transit Intelligence Platform
    
    Person(operator, "Transit Operator", "Manages schedules, views live delays, and monitors fleet performance.")
    System_Ext(opendata, "Open Data Swiss", "Provides GTFS static and real-time transit telemetry protobuf feeds.")
    
    System(platform, "Transit Intelligence Platform", "Ingests RT feeds, runs temporal graph routing, and exposes management APIs and dashboards.")
    
    Rel(operator, platform, "Monitors transit network & calculates routes", "HTTPS / JSON")
    Rel(platform, opendata, "Polls live vehicle positions & trip updates", "Protobuf / HTTP")
```

---

## 2. C4 Container Diagram

The C4 Container diagram details the internal services, workers, and data stores, along with their communication protocols.

```mermaid
C4Container
    title Container diagram for Transit Intelligence Platform
    
    Person(operator, "Transit Operator", "Manages schedules & views live delays.")
    System_Ext(opendata, "Open Data Swiss", "Provides GTFS feeds.")
    
    Container(gateway, "API Gateway / NGINX", "NGINX", "Routes client requests, handles rate limiting and SSL termination.")
    Container(web, "Management Portal", "React + TypeScript + Vite", "Provides the UI for live map tracking and transit reporting.")
    Container(api, "Core API", "Fastify + TypeScript", "Transactional controller, manages users, schedules, configurations, and exposes routing APIs.")
    Container(workers, "Ingestion Worker", "Node.js + TypeScript", "Polls Open Data Swiss every 20-30s, parses protobufs, publishes normalized snapshots to Redpanda.")
    Container(analytics, "Analytics Worker", "Python/Node.js + DuckDB", "Consumes snapshots from Redpanda, joins Postgres metadata, updates temporal graph edge weights in DuckDB.")
    
    ContainerDb(postgres, "Primary DB", "PostgreSQL", "Stores user metadata, active configurations, and static GTFS schedule tables.")
    ContainerDb(redis, "Cache & Sessions", "Redis", "Stores session cache, user tokens, and rate limit states.")
    ContainerDb(duckdb, "Embedded Analytics", "DuckDB File-backed", "Stores local temporal graph representations and calculates optimized routing weights.")
    ContainerDb(redpanda, "Event Ledger", "Redpanda", "Decouples ingestion from analytics by storing immutable chronological snapshot streams.")
    
    Rel(operator, gateway, "Uses", "HTTPS")
    Rel(gateway, web, "Serves static files", "HTTP")
    Rel(gateway, api, "Proxies REST requests", "HTTPS / JSON")
    
    Rel(web, api, "Queries API endpoints", "HTTPS / JSON")
    Rel(api, postgres, "Reads/Writes transactions & static GTFS", "Prisma / PG Client")
    Rel(api, redis, "Caches tokens/sessions", "Redis TCP")
    Rel(api, duckdb, "Queries dynamic routes", "DuckDB Client")
    
    Rel(workers, opendata, "Polls protobuf feeds", "Protobuf / HTTP")
    Rel(workers, redpanda, "Publishes raw/normalized snapshots", "Kafka protocol")
    
    Rel(analytics, redpanda, "Subscribes to snapshot topics", "Kafka protocol")
    Rel(analytics, postgres, "Scans static schedules", "duckdb_postgres_scanner")
    Rel(analytics, duckdb, "Writes edge weights", "DuckDB SQL")
```

---

## 3. Data Flow Diagram (Ingestion Pipeline)

This diagram outlines how real-time data flows from the public Swiss feeds to our operational graph store.

```mermaid
graph LR
    ODS[Open Data Swiss] -->|1. Protobuf Feed| Worker[Ingestion Worker]
    Worker -->|2. Parse Protobuf| Parser[Protobufjs Parser]
    Parser -->|3. Publish Normalized Snapshots| Topic[transit.snapshots.normalized]
    subgraph Redpanda Broker
        Topic
    end
    Topic -->|4. Consume Stream| Analytics[Analytics Worker]
    Analytics -->|5. Read Static Schedule| PG[(PostgreSQL)]
    Analytics -->|6. Calculate Edge Weights| DuckDB[(DuckDB Local File)]
```

---

## 4. Event Flow Diagram (Redpanda Integration)

This diagram focuses on topic topology and the pub-sub separation between services.

```mermaid
graph TD
    subgraph Producers
        IW[Ingestion Worker]
    end
    
    subgraph Redpanda Event Ledger
        T_Raw[transit.snapshots.raw]
        T_Norm[transit.snapshots.normalized]
        T_Deltas[transit.state.deltas]
        T_Metrics[transit.metrics.operational]
    end
    
    subgraph Consumers & Processors
        IW -->|Polls GTFS-RT & Publishes| T_Raw
        Val[Validation Service] -->|Consumes raw, validates, publishes| T_Norm
        T_Raw -.-> Val
        
        Diff[Diff Processor] -->|Consumes normalized, calculates delta, publishes| T_Deltas
        T_Norm -.-> Diff
        
        Eng[Analytics Engine] -->|Consumes deltas, computes graph, publishes| T_Metrics
        T_Deltas -.-> Eng
        
        API[Core API Gateway] -->|Reads operational metrics for UI| T_Metrics
    end
```
