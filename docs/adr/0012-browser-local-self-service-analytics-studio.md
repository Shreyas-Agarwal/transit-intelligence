# ADR 0012: Analytical Delivery Architecture

## Status

Accepted

## Date

2026-06-23

---

# Context

Phase 1 of Transit Intelligence requires exploratory and operational analytics over Zurich GTFS Static datasets.

Initial architectural assumptions considered three possible analytical delivery models:

## Option A — Backend Semantic Layer

```text
Parquet
    ↓
Backend APIs
    ↓
Semantic Layer
    ↓
Frontend
```

Advantages:

* Centralized business logic
* Familiar enterprise architecture
* Easier access control

Disadvantages:

* Additional infrastructure
* Increased operational complexity
* Additional latency
* Reduced frontend flexibility

---

## Option B — Hybrid Architecture

```text
Parquet
    ↓
Backend Semantic Layer
         ↓
Browser Analytics
```

Advantages:

* Shared logic
* Reduced frontend complexity

Disadvantages:

* Duplicate execution environments
* Increased maintenance burden
* Additional synchronization concerns

---

## Option C — Edge-First Analytics

```text
Parquet
    ↓
DuckDB WASM
    ↓
Browser Semantic Layer
    ↓
Visualization
```

Advantages:

* Zero analytical backend
* Reduced infrastructure
* Reduced latency
* Full client-side exploration
* Simplified deployment model

Disadvantages:

* Browser resource constraints
* Potential scalability limitations
* More sophisticated frontend architecture

---

# Decision Drivers

The following questions were identified during Sprint 03:

1. Can DuckDB WASM query Zurich GTFS datasets interactively?
2. Can semantic modeling execute entirely in-browser?
3. Can dashboard generation occur without backend aggregation?
4. Is a backend semantic layer required during Phase 1?

---

# Validation Activities

Sprint 03 implemented:

* DuckDB WASM execution engine
* Browser-local Parquet loading
* Schema discovery
* Semantic relationship model
* Query planner
* Multi-table analytics
* Dashboard widgets
* Cross-highlighting
* Diagnostics instrumentation
* Benchmarking framework

Dataset sizes:

| Artifact   |      Rows |
| ---------- | --------: |
| Stops      |     2,007 |
| Routes     |       261 |
| Trips      |   171,622 |
| Stop Times | 2,698,455 |

Largest artifact:

```text
zurich_stop_times.parquet
≈ 5.7 MB
≈ 2.7 million rows
```

---

# Benchmark Findings

## Overview Queries

Typical latency:

```text
15–35 ms
```

Examples:

* Stop counts
* Route counts
* Trip counts

---

## Aggregation Queries

Typical latency:

```text
150–250 ms
```

Examples:

* Stop utilization
* Trip distributions
* Route summaries

---

## Join Queries

Typical latency:

```text
650–1300 ms
```

Examples:

* Route-stop event aggregation
* Agency-route-trip summaries

---

## Observations

The browser remained responsive during all benchmark scenarios.

No backend aggregation layer was required.

No materialized views were required.

No caching layer was required.

The largest dataset (2.7 million stop-time records) remained analytically usable within acceptable interactive thresholds.

---

# Additional Findings

An unplanned outcome of Sprint 03 was the emergence of a browser-native analytical workbench.

The implemented semantic layer enabled:

* Schema discovery
* Multi-table analytics
* Dashboard composition
* Query generation
* Cross-highlighting
* Diagnostics

This demonstrated that semantic analytics capabilities can be delivered entirely through browser execution.

This capability was not an original Sprint 03 objective.

However, it provides additional evidence supporting browser-resident analytical delivery.

---

# Decision

Transit Intelligence will adopt:

## Edge-First Analytics

```text
Parquet
    ↓
DuckDB WASM
    ↓
Browser Semantic Layer
    ↓
Visualization
```

for all static GTFS analytical workloads in Phase 1.

The semantic layer will reside in the browser.

Backend semantic services will not be introduced during Phase 1.

---

# Consequences

## Positive

* No analytical backend required
* Simplified deployment model
* Lower operational cost
* Reduced latency
* Greater exploratory flexibility
* Faster iteration cycles

---

## Negative

* Browser memory becomes a scalability boundary
* Larger future datasets may require partitioning
* Historical analytics may eventually exceed browser constraints
* Additional frontend architectural complexity

---

# Deferred Decisions

The following remain out of scope for this ADR:

* GTFS-RT analytical delivery
* Historical snapshot analytics
* Network resilience analysis
* Graph analytics execution architecture
* Long-term storage architecture

These topics will be evaluated in later ADRs.
