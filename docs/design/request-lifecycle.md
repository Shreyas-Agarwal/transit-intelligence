# Design Document: Request Lifecycle

This document traces the data lifecycle of the two primary system operations:

1. Swiss GTFS-RT Ingestion Lifecycle (30s Polling & Protobuf parsing)
2. Live Vehicle Location & Routing Pathfinding Retrieval (Dynamic Graph computation)

## 1. Swiss GTFS-RT Ingestion Lifecycle

```mermaid
sequenceDiagram
    autonumber
    participant Feed as Open Data Swiss (HTTP Feed)
    participant Worker as Ingestion Worker
    participant DB as PostgreSQL (Relational metadata)
    participant Duck as DuckDB (Temporal dynamic graph)

    Note over Worker: Ingestion trigger: Cron tick every 30 seconds
    Worker->>Feed: HTTP GET /gtfs-rt/zurich (Fetch Protobuf feed)
    Feed-->>Worker: Return binary Protobuf payload
    Worker->>Worker: Parse Protobuf using standard GTFS-RT schemas

    par Ingestion to Postgres (Transactional & Static metadata)
        Worker->>DB: Log trip delays & active route schedule changes
        DB-->>Worker: Acknowledge writes
    and Ingestion to DuckDB (Dynamic routing weights)
        Worker->>Duck: Append vehicle positions & calculate travel times
        Worker->>Duck: Recalculate dynamic edge weights on temporal graph
        Duck-->>Worker: Acknowledge updates
    end
```

## 2. Live Routing Pathfinding & Location Query Lifecycle

```mermaid
sequenceDiagram
    autonumber
    participant Portal as Management Web App
    participant Gateway as API Gateway (NGINX)
    participant API as Core API
    participant Duck as DuckDB (Embedded Engine)
    participant DB as PostgreSQL (Static Schedules)

    Portal->>Gateway: GET /api/v1/routing/calculate?from=X&to=Y&time=T
    Gateway->>API: Route request

    API->>Duck: Query path optimization matching time-dependent weights
    Note over Duck, DB: DuckDB uses postgres_scanner to scan static PG tables
    Duck->>DB: Join static GTFS schedules (routes, stop_times)
    DB-->>Duck: Return schedule tables
    Duck->>Duck: Execute dynamic Dijkstra/A* path calculation on temporal graph
    Duck-->>API: Return optimized path segments with dynamic delays

    API-->>Gateway: HTTP 200 OK (Route JSON)
    Gateway-->>Portal: Render optimal routes on Operator Map
```
