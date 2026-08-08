# ADR 0011: GTFS Static Preprocessing and Zurich Operational Subset Strategy

## Status

Accepted

## Date

2026-06-21

---

## Context

The Transit Intelligence Platform consumes Swiss GTFS-Realtime (GTFS-RT) feeds as its primary operational data source.

GTFS-RT messages reference static transit entities including:

* Stops
* Trips
* Routes
* Service calendars

These entities are not self-contained within the realtime feed and must be resolved against a corresponding GTFS Static (GTFS-S) dataset.

The Swiss GTFS-S feed contains nationwide timetable information and includes:

| Table          | Approximate Rows |
| -------------- | ---------------: |
| stops          |          102,796 |
| trips          |        1,584,909 |
| routes         |            5,076 |
| stop_times     |       25,132,289 |
| calendar_dates |        8,867,454 |

Directly loading and processing the full nationwide feed for all analytical workloads introduces unnecessary storage, compute, and visualization complexity during the initial phases of platform development.

The project's current operational scope is limited to the Zurich metropolitan transit network.

A preprocessing strategy is therefore required to derive a Zurich-focused subset from the national GTFS feed while preserving future extensibility.

---

## Decision

A dedicated GTFS Static Processing Layer will be introduced.

The preprocessing layer is responsible for:

1. Loading the Swiss GTFS-S feed.
2. Deriving a Zurich operational subset.
3. Generating reusable Parquet artifacts.
4. Persisting metadata describing generated artifacts.
5. Providing static reference data for downstream GTFS-RT enrichment.

The preprocessing pipeline is implemented as an independent subsystem and is executed before realtime ingestion workflows.

---

## Zurich Subset Definition

The initial Zurich subset is derived using stop-name based filtering.

A stop is considered part of the Zurich operational subset when:

```text
stop_name starts with "Zürich"
```

Examples:

```text
Zürich HB
Zürich Oerlikon
Zürich Flughafen, Bahnhof
Zürich Altstetten
```

The resulting subset contains approximately:

```text
2,007 stops
472 unique stop names
```

This approach was selected because:

* It is deterministic.
* It is reproducible.
* It requires no external GIS datasets.
* It naturally captures the Zurich metropolitan transit network.
* It can later be replaced by a more sophisticated geographic strategy.

Future implementations may replace this rule with:

* Fare zone boundaries
* Administrative boundaries
* Spatial polygons
* GIS-based containment tests

without affecting downstream consumers.

---

## Trip Universe Derivation

Trips are derived from stop_times.

A trip belongs to the Zurich subset when:

```text
At least one stop_time references a Zurich stop.
```

Formally:

```text
trip_id ∈ stop_times
WHERE stop_id ∈ zurich_stops
```

This produces approximately:

```text
171,622 trips
```

---

## Route Universe Derivation

Routes are derived from Zurich trips.

A route belongs to the Zurich subset when:

```text
At least one Zurich trip references the route.
```

Formally:

```text
route_id ∈ zurich_trips
```

This produces approximately:

```text
261 routes
```

---

## Trip Classification

Trips are further classified into operational categories.

### Internal Trip

A trip is classified as Internal when:

```text
All stop_times belong to Zurich stops.
```

Formally:

```text
total_stops == zurich_stops
```

### Crossing Trip

A trip is classified as Crossing when:

```text
At least one stop lies outside the Zurich subset.
```

Formally:

```text
total_stops > zurich_stops
```

Observed distribution:

| Classification |  Trips |
| -------------- | -----: |
| Internal       | 73,958 |
| Crossing       | 97,664 |

---

## Route Classification

Routes are classified based on the classifications of their associated trips.

### Internal Route

All trips are Internal.

### Crossing Route

All trips are Crossing.

### Mixed Route

The route contains both Internal and Crossing trips.

Observed distribution:

| Classification | Routes |
| -------------- | -----: |
| Internal       |     59 |
| Crossing       |    171 |
| Mixed          |     31 |

The Mixed classification is retained because it reflects operational reality for services whose trip patterns vary across the timetable.

---

## Artifact Strategy

The preprocessing layer generates reusable Parquet artifacts.

Current artifacts include:

```text
stops/
    zurich_stops.parquet

trips/
    zurich_trip_ids.parquet
    zurich_trips.parquet
    internal_trips.parquet
    crossing_trips.parquet

routes/
    zurich_routes.parquet
    internal_routes.parquet
    crossing_routes.parquet
    mixed_routes.parquet
```

Additional artifacts may be generated for:

```text
stop_times
calendar
calendar_dates
frequencies
agencies
```

All artifacts are immutable outputs of the preprocessing pipeline.

---

## Storage Format Decision

Parquet is selected as the canonical artifact format.

Reasons:

* Columnar storage
* Efficient compression
* Polars compatibility
* DuckDB compatibility
* Apache Arrow interoperability
* Frontend analytical workflows

CSV is used only for source ingestion.

Generated artifacts must be persisted as Parquet.

---

## Processing Technology Decision

Polars is selected as the processing engine.

Reasons:

* Lazy execution
* Predicate pushdown
* Projection pushdown
* High-performance joins
* Native Parquet support
* Tight Arrow integration

Processing must favor:

```text
LazyFrames
Semi joins
Single collect operations
```

and avoid:

```text
Python loops
Row iteration
to_list() on large datasets
```

---

## Consequences

### Positive

* Reduces network size before realtime processing.
* Creates reproducible static artifacts.
* Establishes a reusable semantic layer for GTFS-RT enrichment.
* Supports future DuckDB and Arrow workflows.
* Provides a clear separation between static and realtime processing.

### Negative

* Zurich boundaries are heuristic rather than geographic.
* Static artifacts require periodic regeneration.
* Additional storage is required for derived datasets.

---

## Future Work

### Static Feed Automation

Implement scheduled GTFS-S refresh workflows.

### Geographic Boundaries

Evaluate replacement of stop-name filtering with GIS-based definitions.

### Semantic Layer

Construct higher-order network entities:

* Transit graph
* Route corridors
* Transfer networks
* Service frequency models

### Realtime Enrichment

Join GTFS-RT events against generated static artifacts.

### Visualization

Expose static artifacts through:

* Apache Arrow
* DuckDB
* Apache ECharts
* MapLibre

to support interactive transit network exploration.
