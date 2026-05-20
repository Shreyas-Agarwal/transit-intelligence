# Transit Intelligence Platform

A high-performance, production-grade transit intelligence platform built using a modular monorepo architecture. This project serves as a showcase of systems engineering, scalability thinking, and mature documentation practices.

## Project Structure

```text
transit-intelligence/
├── apps/
│   ├── web/          # React/TypeScript/Vite Frontend Portal
│   ├── api/          # Express/TypeScript Core REST API
│   ├── workers/      # Python/TypeScript telemetry ingestion & processing workers
│   ├── gateway/      # API Gateway (NGINX proxy initially)
│   └── cli/          # Command-Line administrative tooling
├── packages/
│   ├── shared-config/# ESLint, Prettier, and TypeScript base configurations
│   ├── shared-types/ # Shared domain data types and schemas
│   ├── shared-logger/# Structured logger with correlation and request tracing
│   └── shared-errors/# Centralized standard error boundary codes and handler types
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

## Tech Stack

- **Monorepo Manager:** `pnpm` workspaces + Turborepo (`turbo`)
- **Language:** TypeScript (Node.js/React) & Python (Data Engineering/analytics)
- **Frontend:** React, Tailwind CSS, Zustand, TanStack Query, Vite
- **Backend/API:** Node.js, Express, TypeScript, Zod
- **Database (Transactional):** PostgreSQL
- **Caching & Event Bus (Core):** Redis
- **Ingestion & Analytics (Phase 2 Ready):** ClickHouse & Redpanda (Kafka-compatible)

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

3. **Run the development servers:**
   ```bash
   pnpm run dev
   ```

## Architectural Decency & Governance

Every major decision is documented in the Sequential Architectural Decision Records (`docs/adr/`).
Refer to [ARCHITECTURE.md](file:///D:/transit-intelligence/ARCHITECTURE.md) for a block diagram and communications breakdown, and [AGENTS.md](file:///D:/transit-intelligence/AGENTS.md) for guidelines governing human-agent coordination.
