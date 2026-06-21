# Sprint 04 Specification

## Sprint Information

* **Period:** June 22, 2026 – June 28, 2026
* **Theme:** Transit Network Modeling Foundation
* **Branch:** `feat/network-modeling`

---

# Sprint Goal

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

# Background

Sprint 03 validated browser-resident analytical execution and resulted in ADR 0012.

During analytical exploration several data quality and semantic modeling opportunities were discovered:

* Agency naming inconsistencies
* Route classification ambiguity
* Service categorization issues
* Potential stop normalization opportunities

These findings suggest that analytical correctness now becomes more important than geographic correctness.

Before graph analytics can begin, a canonical transit network representation must be established.

---

# Objectives

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

Sprint 04 will establish a documented semantic model.

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
ADR 0013
Transit Network Representation
```

---

## Objective 4 – Network Construction Pipeline

Build the first graph generation pipeline.

---

### Inputs

GTFS Static subset.

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

---

# Sprint 04 Specification

## Sprint Information

* **Period:** June 22, 2026 – June 28, 2026
* **Theme:** Transit Network Modeling Foundation
* **Branch:** `feat/network-modeling`

---

# Sprint Goal

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

# Background

Sprint 03 validated browser-resident analytical execution and resulted in ADR 0012.

During analytical exploration several data quality and semantic modeling opportunities were discovered:

* Agency naming inconsistencies
* Route classification ambiguity
* Service categorization issues
* Potential stop normalization opportunities

These findings suggest that analytical correctness now becomes more important than geographic correctness.

Before graph analytics can begin, a canonical transit network representation must be established.

---

# Objectives

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

Sprint 04 will establish a documented semantic model.

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
ADR 0013
Transit Network Representation
```

---

## Objective 4 – Network Construction Pipeline

Build the first graph generation pipeline.

---

### Inputs

GTFS Static subset.

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

---

## Objective 6 - Pipeline Quality and Benchmarking



---

## Deliverables

### Code

```text
scripts/network_modeling/
```

Containing:

* Network builder
* Graph generation pipeline
* Validation utilities

---

### Artifacts

```text
data/network/
```

Containing:

* nodes.parquet
* edges.parquet

---

### Documentation

```text
docs/design/transit-semantic-model.md
docs/research/gtfs-analytical-profiling.md
```

---

### ADR

```text
ADR 0013
Transit Network Representation
```

---

# Definition of Done

* [ ] Analytical profiling completed
* [ ] Semantic model documented
* [ ] Network representation selected
* [ ] ADR 0013 written
* [ ] Graph construction pipeline implemented
* [ ] nodes.parquet generated
* [ ] edges.parquet generated
* [ ] Degree analysis completed
* [ ] Connected component analysis completed
* [ ] Shortest-path validation completed
* [ ] Documentation completed

---

# Explicitly Out of Scope

* GTFS-RT integration
* Historical analytics
* Browser visualization work
* Analytics Studio enhancements
* Resilience analysis
* Cascading failures
* Operational observability
* Production deployment
* Authentication
* Centrality research beyond degree metrics

---

# Success Criteria

At sprint completion we should be able to answer:

1. What is the canonical transit network representation?
2. How are GTFS entities mapped into a graph?
3. Can graph artifacts be generated reproducibly from GTFS data?
4. Are the generated networks analytically valid?
5. Is the platform ready for advanced network analytics in future sprints?

The primary output of the sprint is ADR 0013.

The graph artifacts exist to establish the foundation for future topology, resilience, and scenario analysis work.

---

## Deliverables

### Code

```text
scripts/network_modeling/
```

Containing:

* Network builder
* Graph generation pipeline
* Validation utilities

---

### Artifacts

```text
data/network/
```

Containing:

* nodes.parquet
* edges.parquet

---

### Documentation

```text
docs/design/transit-semantic-model.md
docs/research/gtfs-analytical-profiling.md
```

---

### ADR

```text
ADR 0013
Transit Network Representation
```

---

# Definition of Done

* [ ] Analytical profiling completed
* [ ] Semantic model documented
* [ ] Network representation selected
* [ ] ADR 0013 written
* [ ] Graph construction pipeline implemented
* [ ] nodes.parquet generated
* [ ] edges.parquet generated
* [ ] Degree analysis completed
* [ ] Connected component analysis completed
* [ ] Shortest-path validation completed
* [ ] Documentation completed
* [ ] pytest configured
* [ ] Data quality validation implemented
* [ ] Graph integrity validation implemented
* [ ] Bencchmark harness implemented
* [ ] Testing strategy documented

---

# Explicitly Out of Scope

* GTFS-RT integration
* Historical analytics
* Browser visualization work
* Analytics Studio enhancements
* Resilience analysis
* Cascading failures
* Operational observability
* Production deployment
* Authentication
* Centrality research beyond degree metrics

---

# Success Criteria

At sprint completion we should be able to answer:

1. What is the canonical transit network representation?
2. How are GTFS entities mapped into a graph?
3. Can graph artifacts be generated reproducibly from GTFS data?
4. Are the generated networks analytically valid?
5. Is the platform ready for advanced network analytics in future sprints?

The primary output of the sprint is ADR 0013.

The graph artifacts exist to establish the foundation for future topology, resilience, and scenario analysis work.
