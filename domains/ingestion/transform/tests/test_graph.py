from __future__ import annotations

import polars as pl

from ingestion.transform.graph import build_transit_graph


def _stops(rows: dict[str, tuple[str, float, float, str]]) -> pl.DataFrame:
    """`rows`: stop_id -> (stop_name, stop_lat, stop_lon, stop_type)."""
    return pl.DataFrame(
        {
            "stop_id": list(rows.keys()),
            "stop_name": [v[0] for v in rows.values()],
            "stop_lat": [v[1] for v in rows.values()],
            "stop_lon": [v[2] for v in rows.values()],
            "stop_type": [v[3] for v in rows.values()],
        }
    )


def _tables(trips: pl.DataFrame, stop_times: pl.DataFrame) -> dict[str, pl.DataFrame]:
    return {"trips": trips, "stop_times": stop_times}


def _edge(edges: pl.DataFrame, source: str, target: str) -> dict | None:
    row = edges.filter(
        (pl.col("source_stop_id") == source) & (pl.col("target_stop_id") == target)
    )
    return None if row.height == 0 else row.to_dicts()[0]


def test_simple_trip_produces_consecutive_directed_edges():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "internal"),
            "C": ("Stop C", 3.0, 3.0, "internal"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T1"],
            "stop_id": ["A", "B", "C"],
            "stop_sequence": [1, 2, 3],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    pairs = set(zip(graph.edges["source_stop_id"], graph.edges["target_stop_id"], strict=True))
    assert pairs == {("A", "B"), ("B", "C")}


def test_reverse_direction_is_a_distinct_edge():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "internal"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1", "T2"], "route_id": ["R1", "R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T2", "T2"],
            "stop_id": ["A", "B", "B", "A"],
            "stop_sequence": [1, 2, 1, 2],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    pairs = set(zip(graph.edges["source_stop_id"], graph.edges["target_stop_id"], strict=True))
    assert pairs == {("A", "B"), ("B", "A")}


def test_boundary_to_external_edge_is_dropped():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "boundary"),
            "C": ("Stop C", 3.0, 3.0, "external"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T1"],
            "stop_id": ["A", "B", "C"],
            "stop_sequence": [1, 2, 3],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    pairs = set(zip(graph.edges["source_stop_id"], graph.edges["target_stop_id"], strict=True))
    assert pairs == {("A", "B")}


def test_external_to_boundary_edge_entering_network_is_kept_the_rest_dropped():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "external"),
            "B": ("Stop B", 2.0, 2.0, "boundary"),
            "C": ("Stop C", 3.0, 3.0, "internal"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T1"],
            "stop_id": ["A", "B", "C"],
            "stop_sequence": [1, 2, 3],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    pairs = set(zip(graph.edges["source_stop_id"], graph.edges["target_stop_id"], strict=True))
    assert pairs == {("B", "C")}


def test_multiple_trips_on_the_same_edge_aggregate_trip_count():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "internal"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1", "T2", "T3"], "route_id": ["R1", "R1", "R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T2", "T2", "T3", "T3"],
            "stop_id": ["A", "B", "A", "B", "A", "B"],
            "stop_sequence": [1, 2, 1, 2, 1, 2],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    edge = _edge(graph.edges, "A", "B")
    assert edge is not None
    assert edge["trip_count"] == 3
    assert graph.edges.height == 1


def test_multiple_routes_on_the_same_edge_aggregate_route_count():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "internal"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1", "T2"], "route_id": ["R1", "R2"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T2", "T2"],
            "stop_id": ["A", "B", "A", "B"],
            "stop_sequence": [1, 2, 1, 2],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    edge = _edge(graph.edges, "A", "B")
    assert edge is not None
    assert edge["route_count"] == 2
    assert edge["trip_count"] == 2


def test_external_stop_never_appears_as_a_node():
    stops = _stops(
        {
            "A": ("Stop A", 1.0, 1.0, "internal"),
            "B": ("Stop B", 2.0, 2.0, "boundary"),
            "C": ("Stop C", 3.0, 3.0, "external"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1", "T1", "T1"],
            "stop_id": ["A", "B", "C"],
            "stop_sequence": [1, 2, 3],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    assert "C" not in graph.nodes["stop_id"].to_list()
    assert sorted(graph.nodes["stop_id"]) == ["A", "B"]


def test_node_attributes_come_from_the_silver_stops_representation():
    stops = _stops(
        {
            "A": ("Zürich HB", 47.378, 8.540, "internal"),
            "B": ("Winterthur", 47.500, 8.724, "boundary"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {"trip_id": ["T1", "T1"], "stop_id": ["A", "B"], "stop_sequence": [1, 2]}
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    row = graph.nodes.filter(pl.col("stop_id") == "A").to_dicts()[0]
    assert row["stop_name"] == "Zürich HB"
    assert row["stop_lat"] == 47.378
    assert row["stop_lon"] == 8.540
    assert row["stop_type"] == "internal"
    assert set(graph.nodes.columns) == {"stop_id", "stop_name", "stop_lat", "stop_lon", "stop_type"}


def test_multiple_entries_and_exits_within_one_trip():
    # X1 -> A -> B -> X2 -> C -> D -> X3: the trip enters/leaves the bounded
    # network twice. Only A->B and C->D (both endpoints non-external) survive;
    # every edge touching an X stop is dropped.
    stops = _stops(
        {
            "X1": ("X1", 0.0, 0.0, "external"),
            "A": ("A", 1.0, 1.0, "internal"),
            "B": ("B", 2.0, 2.0, "boundary"),
            "X2": ("X2", 0.0, 0.0, "external"),
            "C": ("C", 3.0, 3.0, "internal"),
            "D": ("D", 4.0, 4.0, "boundary"),
            "X3": ("X3", 0.0, 0.0, "external"),
        }
    )
    trips = pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]})
    stop_times = pl.DataFrame(
        {
            "trip_id": ["T1"] * 7,
            "stop_id": ["X1", "A", "B", "X2", "C", "D", "X3"],
            "stop_sequence": [1, 2, 3, 4, 5, 6, 7],
        }
    )

    graph = build_transit_graph(_tables(trips, stop_times), stops)

    pairs = set(zip(graph.edges["source_stop_id"], graph.edges["target_stop_id"], strict=True))
    assert pairs == {("A", "B"), ("C", "D")}
    assert sorted(graph.nodes["stop_id"]) == ["A", "B", "C", "D"]
