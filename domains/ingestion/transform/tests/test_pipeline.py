from __future__ import annotations

import polars as pl

import ingestion.transform.pipeline as pipeline_module
from ingestion.transform.pipeline import transform_snapshot
from ingestion.transform.snapshots import Snapshot

from .conftest import write_snapshot


def test_valid_snapshot_writes_silver_artifacts(
    bronze_root, silver_root, graph_root, good_tables, monkeypatch
):
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", good_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))
    monkeypatch.setenv("GTFS_S_SILVER_GRAPH_DIR", str(graph_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert result.validation.passed
    assert result.silver_path == silver_root / "gtfs_fp2026_20260805"
    assert result.artifact_row_counts is not None

    for expected in ("stops", "trip_ids", "trips", "routes", "stop_times"):
        assert expected in result.artifact_row_counts
        assert (result.silver_path / f"{expected}.parquet").exists()

    # Optional tables weren't in the Bronze snapshot -> not in the artifacts.
    assert "calendar" not in result.artifact_row_counts


def test_optional_tables_flow_through_to_silver(
    bronze_root, silver_root, graph_root, good_tables, optional_tables, monkeypatch
):
    tables = {**good_tables, **optional_tables}
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))
    monkeypatch.setenv("GTFS_S_SILVER_GRAPH_DIR", str(graph_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert result.validation.passed
    assert result.artifact_row_counts is not None
    for expected in ("calendar", "calendar_dates", "agencies", "frequencies"):
        assert expected in result.artifact_row_counts
        assert (result.silver_path / f"{expected}.parquet").exists()


def test_invalid_snapshot_writes_no_silver_output(
    bronze_root, silver_root, graph_root, good_tables, monkeypatch
):
    bad_tables = dict(good_tables)
    bad_trips = bad_tables["trips"].clone()
    bad_trips[0, "route_id"] = "R_DOES_NOT_EXIST"
    bad_tables["trips"] = bad_trips
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", bad_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))
    monkeypatch.setenv("GTFS_S_SILVER_GRAPH_DIR", str(graph_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert not result.validation.passed
    assert result.silver_path is None
    assert result.artifact_row_counts is None
    assert result.graph_path is None
    assert not silver_root.exists() or not (silver_root / "gtfs_fp2026_20260805").exists()
    assert not graph_root.exists() or not (graph_root / "gtfs_fp2026_20260805").exists()


def test_valid_snapshot_writes_graph_artifacts_for_the_same_version(
    bronze_root, silver_root, graph_root, good_tables, monkeypatch
):
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", good_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))
    monkeypatch.setenv("GTFS_S_SILVER_GRAPH_DIR", str(graph_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert result.validation.passed
    assert result.graph_path == graph_root / "gtfs_fp2026_20260805"
    assert result.graph_row_counts is not None
    graph_path = result.graph_path
    assert graph_path is not None
    assert (graph_path / "nodes.parquet").exists()
    assert (graph_path / "edges.parquet").exists()
    assert (graph_root / "latest").resolve() == graph_root / "gtfs_fp2026_20260805"

    nodes = pl.read_parquet(graph_path / "nodes.parquet")
    assert "external" not in nodes["stop_type"].to_list()


def test_failed_graph_construction_does_not_touch_static_output_or_advance_latest(
    bronze_root, silver_root, graph_root, good_tables, monkeypatch
):
    # A first, successful run establishes graph `latest` at version 1.
    snapshot_1 = write_snapshot(bronze_root, "gtfs_fp2026_20260729", good_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))
    monkeypatch.setenv("GTFS_S_SILVER_GRAPH_DIR", str(graph_root))
    good_result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260729", path=snapshot_1))
    assert good_result.graph_path is not None
    assert (graph_root / "latest").resolve() == graph_root / "gtfs_fp2026_20260729"

    # The next version's graph construction blows up mid-write.
    def _boom(tables, stops):
        raise RuntimeError("boom")

    # pipeline.py imports the name directly (`from .graph import
    # build_transit_graph`), so it must be patched on pipeline_module itself.
    monkeypatch.setattr(pipeline_module, "build_transit_graph", _boom)

    snapshot_2 = write_snapshot(bronze_root, "gtfs_fp2026_20260805", good_tables)
    bad_result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_2))

    # Static Silver output for the new version is unaffected by the graph failure.
    assert bad_result.validation.passed
    assert bad_result.silver_path == silver_root / "gtfs_fp2026_20260805"
    bad_silver_path = bad_result.silver_path
    assert bad_silver_path is not None
    assert (bad_silver_path / "stops.parquet").exists()

    # But the graph side reports failure and never touched `latest` or
    # published a partial version.
    assert bad_result.graph_path is None
    assert bad_result.graph_row_counts is None
    assert not (graph_root / "gtfs_fp2026_20260805").exists()
    assert (graph_root / "latest").resolve() == graph_root / "gtfs_fp2026_20260729"
