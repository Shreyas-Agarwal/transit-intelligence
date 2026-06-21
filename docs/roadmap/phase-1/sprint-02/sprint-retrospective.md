# Sprint 02 Retrospective

## What Went Well

### Exploration Before Optimization

The sprint focused on understanding the data before building downstream systems.

This prevented premature decisions regarding:

* filtering strategies
* storage formats
* data modeling
* network boundaries

### Redpanda Validation

Running Redpanda locally in WSL significantly reduced operational complexity and accelerated experimentation.

### Static Feed Discovery

The most important outcome of the sprint was discovering that GTFS-RT alone is insufficient.

Realtime events only become meaningful when joined against static transit entities.

This realization shifted the platform architecture in a positive direction.

### Polars Evaluation

Polars performed exceptionally well when processing multi-million-row GTFS datasets.

The resulting architecture remains lightweight while handling production-scale data volumes.

---

## What Did Not Go Well

### Initial Assumptions About Zone 110

The original assumption was that a simple geographic boundary would define the Zurich operational area.

In practice:

* station naming conventions
* metropolitan services
* airport services
* regional rail

made the problem more nuanced than expected.

### Early PostgreSQL Experimentation

Initial attempts to load raw GTFS data into PostgreSQL introduced unnecessary complexity.

Schema mismatches and feed-specific extensions slowed progress.

The team pivoted to direct Polars-based exploration, which proved substantially faster.

### Sprint Scope Expansion

The sprint expanded beyond its original objectives.

Although productive, this created ambiguity regarding sprint completion criteria.

Future exploratory sprints should explicitly allow discovery-driven scope expansion.

---

## Key Architectural Learnings

### GTFS-RT Is Not A Data Model

GTFS-RT is an event stream.

The actual transit model resides within GTFS Static.

### Build Semantic Layers Early

The most valuable outputs were not raw tables but derived artifacts:

* Zurich stops
* Zurich trips
* Internal trips
* Crossing trips
* Route classifications

### Domain Boundaries Emerged Naturally

The project now has clear boundaries:

* Realtime Ingestion
* Static Processing
* Future Enrichment
* Future Analytics

This separation was not fully apparent at sprint start.

---

## Decisions Confirmed

Confirmed architectural choices:

* Redpanda
* Polars
* Parquet
* Polyglot runtime
* Domain-oriented repository structure

No ADR reversals were required.

---

## Actions For Sprint 03

### Primary

* Merge GTFS-RT and GTFS-S through enrichment workflows.
* Build first static transit network explorer.

### Secondary

* Introduce DuckDB analytical workflows.
* Add route and stop-level semantic metrics.

### Deferred

* GIS boundaries
* Graph construction
* Delay propagation analysis
* Resilience scoring

These remain future concerns.

---

## Overall Assessment

Sprint 02 exceeded its original goals.

The platform moved from architectural planning into a functioning data platform with both realtime and static transit capabilities.

This sprint represents the first major foundation milestone of the Transit Intelligence Platform.
