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

### 4. Telemetry Processor (`apps/workers`)

High-throughput ingestion worker handling incoming GPS telemetry.

- **Tech stack:** Node.js (or Python for analytical pipelines).
- **Responsibilities:**
  - Decode IoT telemetry coordinates (coordinates, timestamps, speed).
  - Validate package schema (Zod schema checking).
  - Stream events to Redis (Phase 1) and Kafka (Phase 2).
  - Bulk write coordinate logs to ClickHouse (Phase 2).
