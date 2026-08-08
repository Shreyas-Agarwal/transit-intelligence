from __future__ import annotations

from ingestion.transform.pipeline import transform_snapshot
from ingestion.transform.snapshots import Snapshot

from .conftest import write_snapshot


def test_valid_snapshot_writes_silver_artifacts(bronze_root, silver_root, good_tables, monkeypatch):
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", good_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))

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
    bronze_root, silver_root, good_tables, optional_tables, monkeypatch
):
    tables = {**good_tables, **optional_tables}
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert result.validation.passed
    assert result.artifact_row_counts is not None
    for expected in ("calendar", "calendar_dates", "agencies", "frequencies"):
        assert expected in result.artifact_row_counts
        assert (result.silver_path / f"{expected}.parquet").exists()


def test_invalid_snapshot_writes_no_silver_output(
    bronze_root, silver_root, good_tables, monkeypatch
):
    bad_tables = dict(good_tables)
    bad_trips = bad_tables["trips"].clone()
    bad_trips[0, "route_id"] = "R_DOES_NOT_EXIST"
    bad_tables["trips"] = bad_trips
    snapshot_dir = write_snapshot(bronze_root, "gtfs_fp2026_20260805", bad_tables)
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))

    result = transform_snapshot(Snapshot(version="gtfs_fp2026_20260805", path=snapshot_dir))

    assert not result.validation.passed
    assert result.silver_path is None
    assert result.artifact_row_counts is None
    assert not silver_root.exists() or not (silver_root / "gtfs_fp2026_20260805").exists()
