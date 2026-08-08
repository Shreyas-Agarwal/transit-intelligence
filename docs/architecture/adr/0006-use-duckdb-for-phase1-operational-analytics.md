# ADR 0006: Use DuckDB for Phase 1 Operational Analytics

## Status

Approved

## Context

The Transit Intelligence Platform relies on a dynamic routing and pathfinding engine built around **temporally variable weighted graph theory**. Under this model:

- **Nodes** correspond to transit stops/stations.
- **Edges** correspond to route segments connecting stops.
- **Weights** correspond to transit travel times, which vary dynamically over time based on scheduled departures, real-time traffic congestion, and vehicle delays.

To perform operational route queries (e.g. calculating optimal paths under real-time delays) or generate transit reports, we need to join large static scheduling datasets (static GTFS tables) with live telemetry streams (GTFS-RT). PostgreSQL is not optimized for high-throughput column scans and aggregation. ClickHouse is designated for Phase 2, but launching a dedicated ClickHouse server cluster for early validation complicates local development.

DuckDB is a fast, in-process analytical SQL database designed specifically for column-store aggregation and vector execution.

## Decision

We adopt **DuckDB** as the embedded analytical engine for Phase 1:

1. **Embedded Process Execution:** DuckDB runs directly inside the Node.js/TypeScript ingestion and API processes as a dependency, bypassing networking overhead and cluster administration.
2. **Postgres Integration:** We will use DuckDB's `postgres_scanner` extension or standard logical synchronization to access static GTFS tables stored in PostgreSQL.
3. **Temporal Weight Processing:** We will leverage DuckDB's advanced analytics, window functions, and spatial capabilities to calculate and update the dynamic edge weights of our transit graphs based on live delay values polled from GTFS-RT feeds.

## Consequences

- **Pros:**
  - **Extreme In-Process Speed:** DuckDB processes millions of rows in milliseconds using vectorized execution.
  - **Zero Administration:** No separate database processes to configure, run, or maintain in development.
  - **Standard SQL:** Uses standard postgres-compatible SQL, ensuring that queries can be ported to ClickHouse in Phase 2.
  - **Parquet/File Compatibility:** Allows saving snapshots of temporal graph states directly to compressed Parquet files for backup or analysis.
- **Cons:**
  - **Single-Writer Constraint:** As an in-process engine, write access is restricted to a single process at a time.
  - **Ephemeral Scaling:** Storage is restricted to local volumes, requiring migration to ClickHouse (Phase 2) when scaling horizontally.
