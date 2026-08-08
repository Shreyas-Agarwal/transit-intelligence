# Sprint 05 Specification

## Sprint Information

* **Period:** August 10, 2026 – August 16, 2026
* **Theme:** Transit Network Modeling Foundation (carried over from Sprint 04)
* **Branch:** `feat/network-modeling`

---

## Carryover Notice

Sprint 05 is Sprint 04, unchanged in substance, run again.

Sprint 04 was specified with this exact theme and these exact five objectives. It did not deliver any of them: the sprint went dark for 39 days and the time was spent on an unplanned ingestion-layer rebuild instead, with no re-scoping decision ever recorded against the sprint-04 spec. See:

* `docs/roadmap/phase-1/sprint-04/closure-report.md` — objective-by-objective accounting of what was and wasn't delivered.
* `docs/roadmap/phase-1/sprint-04/sprint-retrospective.md` — how the deviation happened and why it's being treated as a one-off, not a pattern.

Nothing below is new work. It is Sprint 04's specification carried forward verbatim, with three corrections:

1. **Branch name.** Sprint 04 named `feat/network-modeling` but no such branch was ever created. This sprint claims it for real.
2. **ADR number.** Sprint 04 reserved ADR 0013 for the network representation decision (Objective 3). ADR 0013 was spent during the off-plan work on an unrelated decision ("Adopt Domain-First Workspace Organization"). This sprint's network representation ADR is renumbered to **ADR 0014**. Whoever picks up Sprint 06 should confirm 0014 is still free before writing it — the same check that was skipped last time.
3. **Foundation.** The ingestion rebuild that displaced Sprint 04 (Rust static + realtime ingestion, Bronze/Silver transform pipeline, `gtfs_s` retirement) is now merged. The Zurich GTFS subset this sprint operates on is produced by that pipeline rather than by the standalone `gtfs_s` scripts sprint-04 would have used. This does not change any objective below — it changes where the input Parquet files come from.

If Sprint 05 also fails to deliver these objectives, that is no longer a one-off — open that conversation explicitly rather than writing a Sprint 06 carryover notice that just repeats this one.

---

## Sprint Goal

Transform the Zurich GTFS subset from a collection of transit tables into a formally modeled transit network.

The objective is not advanced graph analytics.

The objective is to establish:

* Analytical entity definitions
* Semantic normalization
* Network topology construction
* Graph-ready data products

This sprint exists to validate a second architectural hypothesis:

```text
GTFS Static
    ↓
Semantic Modeling
    ↓
Network Construction
    ↓
Graph Artifacts
    ↓
Network Analytics
```

---

## Background

Sprint 03 validated browser-resident analytical execution and resulted in ADR 0012.

During analytical exploration several data quality and semantic modeling opportunities were discovered:

* Agency naming inconsistencies
* Route classification ambiguity
* Service categorization issues
* Potential stop normalization opportunities

These findings suggest that analytical correctness now becomes more important than geographic correctness.

Before graph analytics can begin, a canonical transit network representation must be established.

These findings and this rationale are unchanged since Sprint 04 was first specified. The Zurich subset has since been re-platformed onto the Rust ingestion pipeline's Bronze → Silver output, but the underlying data quality questions below were never investigated and remain exactly as open as they were.

---

## Objectives

## Objective 1 – Analytical Data Profiling

Perform analytical profiling across the Zurich GTFS subset.

### Focus Areas

#### Agencies

Evaluate:

* Naming consistency
* Operational variants
* Replacement services
* Agency grouping opportunities

#### Routes

Evaluate:

* Route type distributions
* Service classifications
* Route naming conventions
* Missing or ambiguous values

#### Stops

Evaluate:

* Duplicate names
* Parent-child relationships
* Station vs platform modeling
* Coordinate anomalies

#### Trips

Evaluate:

* Service frequency distributions
* Route utilization
* Outlier trips

---

### Deliverables

Produce:

```text
docs/research/gtfs-analytical-profiling.md
```

Document:

* Findings
* Proposed normalizations
* Deferred issues

---

## Objective 2 – Semantic Model v1

Formalize the transit semantic layer.

Current relationships exist in code.

Sprint 05 will establish a documented semantic model.

---

### Deliverables

Create:

```text
docs/design/transit-semantic-model.md
```

Define:

#### Entities

* Agencies
* Routes
* Trips
* Stops
* Stop Times

#### Keys

* Primary Keys
* Foreign Keys

#### Relationships

Examples:

```text
Agency
    ↓
Routes
    ↓
Trips
    ↓
Stop Times
    ↓
Stops
```

---

## Objective 3 – Network Representation Design

Define the canonical network model used by Transit Intelligence.

---

### Key Questions

What is a node?

What is an edge?

What relationships are represented?

What information is retained?

---

### Candidate Models

#### Stop Graph

```text
Stop
  ↔
Stop
```

Based on stop sequence traversal.

---

#### Station Graph

```text
Station
  ↔
Station
```

Collapsed platform model.

---

#### Route Graph

```text
Route
  ↔
Route
```

Transfer-based model.

---

### Decision

Select one canonical model for Phase 1.

Document rationale.

---

### Deliverable

```text
ADR 0014
Transit Network Representation
```

---

## Objective 4 – Network Construction Pipeline

Build the first graph generation pipeline.

---

### Inputs

GTFS Static subset (now sourced from `data/silver/static/latest`, produced by the ingestion domain's Bronze → Silver transform pipeline).

---

### Outputs

Generate:

```text
nodes.parquet
edges.parquet
```

---

### Node Attributes

Examples:

* stop_id
* stop_name
* latitude
* longitude

---

### Edge Attributes

Examples:

* source_stop_id
* target_stop_id
* route_count
* trip_count

---

### Success Criteria

Graph artifacts can be regenerated from GTFS source data.

---

## Objective 5 – Baseline Network Analytics

Implement foundational graph metrics.

---

### Metrics

#### Degree

Identify highly connected nodes.

---

#### Connected Components

Identify disconnected subnetworks.

---

#### Shortest Path Validation

Verify graph traversal correctness.

---

### Explicitly Excluded

* Betweenness centrality
* Resilience analysis
* Failure propagation
* Cascading disruption modeling
* Recovery optimization

These will be evaluated later.
