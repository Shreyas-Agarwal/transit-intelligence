"""Canonical transit graph (v1) — the first graph representation of the
network, established as a stable base to inspect and validate before station
collapsing, transfer modelling, temporal edge weights, or route-level
abstractions land on top of it.

The model:

* One **node** per non-`external` stop. Node source is the Silver `stops`
  artifact (`subset.py`'s `stop_type` classification): `internal` and
  `boundary` stops are materialized as nodes, `external` stops never appear.
* One **directed edge** per `(source_stop_id, target_stop_id)` pair observed
  as a consecutive `stop_sequence` traversal within any GTFS trip — trip
  order is authoritative, nothing is inferred from geometry or distance.
  Edges are aggregated across every trip/route that makes that exact
  traversal (`route_count`, `trip_count`); A -> B and B -> A are independent
  edges, never collapsed into one.
* **Bounded**: an edge is kept only when both its source and target are
  non-external. A trip segment that touches an `external` stop on either end
  is dropped, which is what keeps `external` stops out of the graph even
  though edges are derived from the full nationwide `trips` / `stop_times`
  (not the Zurich-only `internal_trips` subset `subset.py` builds) — using
  the full trip universe here, rather than the already-filtered one, is what
  makes multi-entry/exit trips and boundary-to-boundary traversals (via a
  trip that never itself touches an `internal` stop) come out correctly.
"""

from __future__ import annotations

from dataclasses import dataclass

import polars as pl

NODE_COLUMNS = ("stop_id", "stop_name", "stop_lat", "stop_lon", "stop_type")


@dataclass
class TransitGraph:
    nodes: pl.DataFrame
    edges: pl.DataFrame


def _build_nodes(stops: pl.DataFrame) -> pl.DataFrame:
    return stops.filter(pl.col("stop_type") != "external").select(list(NODE_COLUMNS))


def _build_edges(tables: dict[str, pl.DataFrame], node_ids: pl.DataFrame) -> pl.DataFrame:
    trip_routes = tables["trips"].lazy().select("trip_id", "route_id")

    consecutive_pairs = (
        tables["stop_times"]
        .lazy()
        .select("trip_id", "stop_id", "stop_sequence")
        .sort(["trip_id", "stop_sequence"])
        .with_columns(pl.col("stop_id").shift(-1).over("trip_id").alias("target_stop_id"))
        .rename({"stop_id": "source_stop_id"})
        .drop_nulls("target_stop_id")  # trip's last stop has no successor
        .select("trip_id", "source_stop_id", "target_stop_id")
    )

    bounded_pairs = consecutive_pairs.join(
        node_ids.lazy().rename({"stop_id": "source_stop_id"}), on="source_stop_id", how="semi"
    ).join(node_ids.lazy().rename({"stop_id": "target_stop_id"}), on="target_stop_id", how="semi")

    return (
        bounded_pairs.join(trip_routes, on="trip_id", how="left")
        .group_by(["source_stop_id", "target_stop_id"])
        .agg(
            pl.col("route_id").n_unique().alias("route_count"),
            pl.col("trip_id").n_unique().alias("trip_count"),
        )
        .collect()
    )


def build_transit_graph(tables: dict[str, pl.DataFrame], stops: pl.DataFrame) -> TransitGraph:
    """`tables` is the full Bronze snapshot (as loaded by
    `pipeline._load_tables`) and `stops` is the same snapshot's already
    `stop_type`-classified Silver `stops` artifact (`subset.py`).
    """
    nodes = _build_nodes(stops)
    edges = _build_edges(tables, nodes.select("stop_id"))
    return TransitGraph(nodes=nodes, edges=edges)
