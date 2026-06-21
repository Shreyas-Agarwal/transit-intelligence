# Sprint 03 Specification

## Sprint Information

* **Period:** June 21, 2026 – June 23, 2026
* **Theme:** Network Explorer & Edge Analytics Validation
* **Branch:** `feat/network-explorer`

---

# Sprint Goal

Build the first user-facing Transit Network Explorer using the Zurich GTFS Static subset.

The objective is not UI design.

The objective is to validate that browser-local analytics can support Zurich-scale transit datasets without requiring a backend semantic layer.

This sprint exists to validate a working architectural hypothesis:

```text
Parquet
  ↓
DuckDB WASM
  ↓
Browser Semantic Layer
  ↓
Apache ECharts
  ↓
Interactive Analytics
```

---

# Background

Sprint 02 produced a complete Zurich GTFS Static subset.

Generated artifacts include:

| Artifact   |      Rows |
| ---------- | --------: |
| Stops      |     2,007 |
| Trips      |   171,622 |
| Routes     |       261 |
| Stop Times | 2,698,455 |
| Agencies   |        18 |

Largest artifact:

```text
zurich_stop_times.parquet
≈ 5.7 MB
≈ 2.7 million rows
```

Given current dataset sizes, browser-local execution appears viable.

Sprint 03 validates this assumption.

---

# Objectives

## Objective 1 – Browser Analytics Foundation

Establish browser-local analytical execution.

### Tasks

* Configure DuckDB WASM
* Load Parquet artifacts directly in browser
* Execute SQL queries locally
* Benchmark startup time
* Benchmark aggregation latency

### Success Criteria

Demonstrate interactive analytical queries over:

* Routes
* Trips
* Stop Times
* Agencies

without backend aggregation.

---

## Objective 2 – Network Explorer Prototype

Build a single analytical dashboard.

The goal is architectural validation, not feature completeness.

### Required Views

#### Network Overview

Display:

* Total Stops
* Total Trips
* Total Routes
* Total Agencies
* Total Stop Times

#### Service Composition

Display:

* Internal Trips
* Crossing Trips

Display:

* Internal Routes
* Crossing Routes
* Mixed Routes

#### Agency Breakdown

Display:

* Route counts
* Trip counts

---

## Objective 3 – Spatial Visualization

Display Zurich transit stops.

### Technology

* MapLibre

### Required Features

* Pan
* Zoom
* Stop rendering
* Basic clustering

No route geometry required.

No realtime data required.

---

## Objective 4 – Semantic Layer Validation

Evaluate whether SQL views inside DuckDB WASM can serve as the semantic layer.

Candidate views:

### route_summary

* route count
* trip count
* stop count

### agency_summary

* route count
* trip count

### network_summary

* global metrics

Determine:

* what should remain precomputed
* what should remain query-driven

---

## Objective 5 – ADR 0012

Produce a formal architectural recommendation.

### ADR 0012

Analytical Delivery Architecture

Question:

Can Zurich-scale static transit analytics execute entirely in-browser?

Decision options:

* Edge-first analytics
* Backend semantic layer
* Hybrid execution model

---

# Deliverables

## Application

`apps/network-explorer`

### Technology Stack

* React
* TypeScript
* Vite
* DuckDB WASM
* Apache ECharts
* MapLibre

---

## Documentation

### ADR 0012

Analytical Delivery Architecture

### Design Document

Network Explorer Architecture

### Benchmark Notes

Document:

* load times
* memory usage
* query latency
* rendering performance

---

# Definition of Done

* [ ] DuckDB WASM configured
* [ ] Parquet artifacts loaded in browser
* [ ] SQL execution demonstrated
* [ ] Overview dashboard implemented
* [ ] Service composition dashboard implemented
* [ ] Agency dashboard implemented
* [ ] Stop map implemented
* [ ] Performance benchmarks captured
* [ ] ADR 0012 written
* [ ] Architectural recommendation finalized

---

# Explicitly Out of Scope

* GTFS-RT enrichment
* Historical analytics
* Delay propagation
* PostgreSQL integration
* Authentication
* Production UI design
* Mobile responsiveness
* Graph analytics
* Resilience analysis
* Operational observability

---

# Success Criteria

At sprint completion we should be able to answer:

1. Can DuckDB WASM query Zurich GTFS artifacts interactively?
2. Can Apache ECharts render analytical views directly from browser-side SQL?
3. Can MapLibre handle Zurich stop visualizations without preprocessing?
4. Is a backend semantic layer required for Phase 1?
5. Should future static analytics default to edge execution?

The primary output of the sprint is ADR 0012.

The dashboard exists to generate evidence for that decision.
