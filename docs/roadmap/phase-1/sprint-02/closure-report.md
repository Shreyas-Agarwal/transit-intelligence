# Sprint 02 Closure Report

## Sprint Information

* **Sprint:** Sprint 02
* **Period:** June 12, 2026 – June 21, 2026
* **Theme:** GTFS-RT Ingestion Foundation
* **Branch:** feat/gtfs-ingestion; feat/gtfs-static
* **Status:** Completed

---

# Sprint Goal

Establish the foundational data acquisition layer for the Transit Intelligence Platform.

The original scope focused on:

* GTFS-RT feed exploration
* GTFS-RT decoding
* Redpanda publishing
* Architecture documentation

During implementation, the sprint expanded to include GTFS Static preprocessing after discovering that realtime transit events cannot be meaningfully interpreted without static stop, route, trip, and calendar context.

---

# Delivered

## GTFS-RT Ingestion Foundation

### Feed Exploration

Completed:

* GTFS-RT feed acquisition
* Protobuf decoding
* Feed inspection tooling
* Feed structure documentation

Observed feed characteristics:

* Combined Swiss GTFS-RT endpoint
* Trip Update entities present
* Vehicle Positions absent
* Alerts absent
* Full dataset snapshots

### Redpanda Integration

Completed:

* KafkaJS producer implementation
* Topic bootstrap workflow
* Local Redpanda development environment
* Raw event publication pipeline

Topics established:

* transit.snapshots.raw
* transit.snapshots.normalized
* transit.state.deltas
* transit.metrics.operational

### Architecture Documentation

Completed:

* GTFS-RT feed structure analysis
* Domain mapping
* Topic configuration
* Zone filtering strategy
* Operational runbooks

---

## GTFS Static Processing Layer

A new GTFS Static domain was introduced.

### GTFS-S Exploration

Completed:

* Swiss GTFS feed inspection
* Dataset profiling
* Zurich boundary investigation
* Stop density analysis

### Zurich Operational Subset

Generated artifacts:

| Artifact                 |      Rows |
| ------------------------ | --------: |
| Zurich Stops             |     2,007 |
| Zurich Trips             |   171,622 |
| Zurich Routes            |       261 |
| Zurich Stop Times        | 2,698,455 |
| Zurich Calendar Services |    13,840 |
| Zurich Agencies          |        18 |

### Service Classification

Trips classified as:

| Type     |  Count |
| -------- | -----: |
| Internal | 73,958 |
| Crossing | 97,664 |

Routes classified as:

| Type     | Count |
| -------- | ----: |
| Internal |    59 |
| Crossing |   171 |
| Mixed    |    31 |

### Artifact Generation Framework

Implemented:

* Polars-based preprocessing engine
* Artifact writer
* Manifest generation
* Metadata tracking
* Reproducible pipeline execution

---

## Repository Modernization

Completed:

* Polyglot runtime architecture
* uv-based Python dependency management
* Domain-oriented repository structure
* Shared tooling standardization

---

# Deliverables Produced

## Applications

* apps/ingestion

## Domains

* domains/gtfs_s

## Documentation

* Architecture documentation
* Runbooks
* Design documents
* ADRs

## Data Artifacts

* Zurich GTFS Static subset
* Manifest metadata
* Exploratory notebook

---

# Definition of Done Review

| Item                       | Status   |
| -------------------------- | -------- |
| GTFS-RT feed exploration   | Complete |
| GTFS-RT decode pipeline    | Complete |
| Redpanda publishing        | Complete |
| Topic strategy             | Complete |
| Feed documentation         | Complete |
| Operational runbooks       | Complete |
| Static feed investigation  | Complete |
| Zurich subset generation   | Complete |
| Static artifact pipeline   | Complete |
| Architecture documentation | Complete |

---

# Sprint Outcome

Sprint 02 successfully established the platform's first operational data layer.

The project now possesses:

* A realtime acquisition pipeline
* A message streaming backbone
* A Zurich-focused static transit dataset
* Reusable analytical artifacts

These capabilities form the foundation for all subsequent enrichment, observability, analytics, and visualization work.
