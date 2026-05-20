# Design Document: System Context & Boundaries

This document defines the functional boundaries, runtime contexts, and service responsibilities of the Transit Intelligence Platform.

## Service Boundaries

### 1. API Gateway (`apps/gateway`)

Acts as the single entrypoint for all incoming HTTP/gRPC requests.

- **Tech stack:** NGINX or reverse proxy.
- **Responsibilities:**
  - Route routing (path mapping to API services and frontend assets).
  - Rate limiting (preventing DDoS on telemetry ports).
  - SSL/TLS termination.

### 2. Management Portal (`apps/web`)

The web application interface for operators, transit managers, and dispatchers.

- **Tech stack:** React + TypeScript + Tailwind CSS (Vite builder).
- **Responsibilities:**
  - Authenticating and authorized sessions.
  - Live Map rendering (GPS coordinates tracking).
  - Transit reports, metrics dashboard, and configuration grids.

### 3. Core API (`apps/api`)

The transactional state controller.

- **Tech stack:** Node.js + Express + TypeScript + Prisma/PgPool.
- **Responsibilities:**
  - Account/User administration.
  - Route planning, static schedule assignments, and vehicle inventory management.
  - CRUD operations over alerts and configurations.

### 4. Swiss GTFS-RT Ingestion Worker (`apps/workers`)

High-throughput polling worker retrieving live transit feeds.

- **Tech stack:** Node.js + TypeScript + `protobufjs`.
- **Responsibilities:**
  - Poll the Swiss Open Data HTTP feeds for GTFS-RT updates every 30 seconds.
  - Parse binary protobuf payloads to structured JSON records.
  - Push parsed positions, trip delays, and schedule adjustments to the Redis cache stream and analytical store.

### 5. Graph Analytics Engine (Embedded)

Calculates optimal routes and delays using temporally variable weighted graph theory.

- **Tech stack:** Node.js + DuckDB (embedded file-backed column store).
- **Responsibilities:**
  - Query static timetables from PostgreSQL and join them with dynamic delay variables in DuckDB.
  - Reconstruct the transit network as a weighted graph, where edge weights dynamically represent travel times as a function of the time of day, congestion metrics, and active vehicle delays.
  - Expose fast relational SQL interfaces to the Core API for operational pathfinding queries.
