# Design Document: Request Lifecycle

This document traces the data lifecycle of the two primary system operations:

1. Vehicle Telemetry Ingestion (High throughput, time-series)
2. Live Vehicle Location Retrieval (Low latency, read)

## 1. Vehicle Telemetry Ingestion Lifecycle

```mermaid
sequenceDiagram
    autonumber
    participant Vehicle as IoT/Vehicle Tracker
    participant Gateway as API Gateway (NGINX)
    participant Worker as Telemetry Worker
    participant Broker as Message Broker (Redis/Kafka)
    participant AnalyticalDB as Analytical DB (ClickHouse)

    Vehicle->>Gateway: POST /api/v1/telemetry (GPS data)
    Gateway->>Worker: Route request
    Worker->>Worker: Validate message using shared Zod schema
    Worker->>Broker: Publish RAW_TELEMETRY event
    Worker->>Gateway: Return 202 Accepted (Non-blocking)
    Gateway->>Vehicle: HTTP 202 Accepted

    Note over Broker, AnalyticalDB: Asynchronous processing
    Worker->>Broker: Consume RAW_TELEMETRY (Batch buffer)
    Worker->>AnalyticalDB: Insert telemetry batch (ClickHouse)
```

## 2. Live Vehicle Location Query Lifecycle

```mermaid
sequenceDiagram
    autonumber
    participant Portal as Management Web App
    participant Gateway as API Gateway (NGINX)
    participant API as Core API
    participant Cache as Redis Cache
    participant DB as PostgreSQL (Metadata)

    Portal->>Gateway: GET /api/v1/vehicles/active
    Gateway->>API: Route request
    API->>Cache: Query active vehicle locations (from Redis Cache)
    alt Cache Hit
        Cache-->>API: Return coordinates JSON
    else Cache Miss
        API->>DB: Query last known states (fallback)
        DB-->>API: Return states
        API->>Cache: Populate cache (TTL: 10s)
    end
    API-->>Gateway: HTTP 200 OK (Coordinates JSON)
    Gateway-->>Portal: Render vehicle markers on Map
```
