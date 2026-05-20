# ADR 0004: Database Strategy - PostgreSQL & ClickHouse

## Status

Approved

## Context

Transit systems generate two distinct types of data:

1. Relational, transactional states (users, roles, route plans, billing, vehicle registrations).
2. Time-series, analytics logs (GPS coordinate history, diagnostic telemetry, check-ins).
   Storing both in a single relational store causes slow queries, write bottlenecks, and massive storage inflation.

## Decision

We adopt a multi-database strategy:

- **PostgreSQL** as the primary transactional database (relational state, integrity constraints).
- **ClickHouse** as the target analytics engine. Telemetry coordinates will be buffered in Kafka/Redpanda and written in batches into ClickHouse.
- Direct operational reads/writes for fleet statuses bypass ClickHouse, referencing PostgreSQL or Redis cache states. ClickHouse is strictly for analytical aggregating queries and reporting.

## Consequences

- **Pros:**
  - PostgreSQL handles relational queries with transaction safety.
  - ClickHouse handles compression and fast queries over billions of telemetry rows.
  - Decoupled read/write performance.
- **Cons:**
  - Need to manage two database engines.
  - Requires maintaining schemas and migration pipelines in both databases.
