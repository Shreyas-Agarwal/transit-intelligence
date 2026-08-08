# ADR 0007: Ingest Swiss GTFS-RT Datasets via 30s Polling

## Status

Approved

## Context

To implement real-time operational routing and transit delay tracking, we need live location and schedule status updates. Switzerland provides comprehensive public transportation resources (under the Open Data Swiss initiative) through the General Transit Feed Specification Realtime (GTFS-RT) format, specifically for agencies like VBZ (Zurich).

GTFS-RT data is exposed as binary-encoded Protocol Buffer (protobuf) feeds covering three main entities:

1. **Trip Updates:** Delays, cancellations, and schedule alterations.
2. **Vehicle Positions:** Live geographic coordinates, speeds, and timestamps.
3. **Alerts:** Dynamic announcements and disruptions.

Because these files update dynamically on the provider side every 20-30 seconds, we must pull them frequently enough to keep dynamic routing graphs accurate, without causing rate-limit bans or CPU bottlenecks.

## Decision

We establish the following Swiss GTFS-RT ingestion architecture:

1. **Ingestion Worker:** Implement a dedicated poll-based worker process in our workspace that wakes up every 30 seconds to fetch the Swiss GTFS-RT protobuf feed.
2. **Protobuf Parsing:** Utilize `protobufjs` with the standard GTFS-RT protobuf schema definitions to parse the binary response into structured JSON objects.
3. **Delayed Class Instantiation:** We will hold off on creating static TypeScript domain classes until we inspect the concrete fields and variations of the Zurich feed schemas. For now, data will be parsed into raw TypeScript interfaces mapping the GTFS-RT structure.
4. **Targeted Dispatches:**
   - Write persistent schedules and static metadata to PostgreSQL.
   - Stream raw location coordinates and delay updates directly into **DuckDB** tables to calculate temporally variable weighted graph parameters.

## Consequences

- **Pros:**
  - **High-Fidelity Delays:** 30s updates capture traffic delays in real time, keeping graph weights accurate.
  - **Standardized Ingestion:** Adherence to GTFS-RT ensures compatibility with any transit agency worldwide if we scale beyond Switzerland.
  - **Relational Independence:** Heavy write operations bypass Postgres transactional tables, keeping them fast.
- **Cons:**
  - **Network Dependency:** Platform reliability depends on the availability of the Open Data Swiss endpoints.
  - **Data Volatility:** Requires clean-up policies to prune or archive raw GPS coordinates in DuckDB to avoid local storage exhaustion.
