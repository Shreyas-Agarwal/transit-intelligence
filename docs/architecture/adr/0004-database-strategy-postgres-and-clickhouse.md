# ADR 0004: Database Strategy - PostgreSQL, DuckDB & ClickHouse

## Status

Approved (Amended to include DuckDB in Phase 1)

## Context

Transit platforms require storing two distinct categories of data:

1. Transactional and static relational assets (users, permissions, static GTFS stops/schedules, configuration).
2. Large volume time-series, operational logs, and live delay telemetry (GTFS-RT, vehicle positions, travel delays).

Running massive analytical aggregations (like calculating temporal-weighted path routing or computing historical delay profiles) inside PostgreSQL introduces write locks, slow queries, and storage expansion. However, spinning up and operating a distributed ClickHouse instance in the early development phase (Phase 1) introduces unnecessary infrastructure complexity and high maintenance overhead.

## Decision

We adopt an evolutionary multi-database strategy:

1. **PostgreSQL** remains the primary transactional store and the authority for static scheduling schemas (static GTFS tables).
2. **DuckDB** is adopted as the Phase 1 analytics database. It is embedded directly within the application workers as a zero-configuration, file-backed column store. DuckDB joins static GTFS schedules from Postgres with dynamic GTFS-RT updates in-memory to compute dynamic path routing and run operational reports.
3. **ClickHouse** is designated as the Phase 2 analytics scaling target. As data size grows, ClickHouse and Redpanda will replace the local DuckDB files to support large-scale historical analytics and machine learning model inputs.

## Consequences

- **Pros:**
  - Relational transaction safety and data integrity are handled by PostgreSQL.
  - Zero-maintenance embedded setup for DuckDB in Phase 1, offering fast SQL column aggregations without separate server clusters.
  - Clear progression path: schemas and SQL queries developed in DuckDB map cleanly to ClickHouse for Phase 2 scaling.
- **Cons:**
  - Need to support DuckDB file persistence and clean up disk logs.
  - Must manage relational sync between PostgreSQL and DuckDB during Phase 1 processing.
