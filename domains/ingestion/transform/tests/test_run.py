from __future__ import annotations

import pytest

from ingestion.transform.run import run

from .conftest import write_snapshot


def test_run_latest_processes_only_the_latest_snapshot(
    bronze_root, silver_root, good_tables, monkeypatch
):
    write_snapshot(bronze_root, "gtfs_fp2026_20260729", good_tables)
    write_snapshot(bronze_root, "gtfs_fp2026_20260805", good_tables)
    (bronze_root / "latest").symlink_to("gtfs_fp2026_20260805")
    monkeypatch.setenv("GTFS_S_RAW_DIR", str(bronze_root))
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))

    results = run(mode="latest")

    assert [r.snapshot.version for r in results] == ["gtfs_fp2026_20260805"]
    assert results[0].validation.passed
    assert results[0].silver_path == silver_root / "gtfs_fp2026_20260805"
    assert (silver_root / "latest").resolve() == silver_root / "gtfs_fp2026_20260805"


def test_run_replay_processes_every_snapshot_independently(
    bronze_root, silver_root, good_tables, monkeypatch
):
    write_snapshot(bronze_root, "gtfs_fp2026_20260729", good_tables)
    bad_tables = dict(good_tables)
    bad_trips = bad_tables["trips"].clone()
    bad_trips[0, "route_id"] = "R_DOES_NOT_EXIST"
    bad_tables["trips"] = bad_trips
    write_snapshot(bronze_root, "gtfs_fp2026_20260805", bad_tables)
    (bronze_root / "latest").symlink_to("gtfs_fp2026_20260805")
    monkeypatch.setenv("GTFS_S_RAW_DIR", str(bronze_root))
    monkeypatch.setenv("GTFS_S_SILVER_DIR", str(silver_root))

    results = run(mode="replay")

    assert [r.snapshot.version for r in results] == [
        "gtfs_fp2026_20260729",
        "gtfs_fp2026_20260805",
    ]
    assert results[0].validation.passed
    assert not results[1].validation.passed
    # One bad historical snapshot doesn't stop the rest of the run.
    assert len(results) == 2
    # The bad snapshot never got Silver output written for it.
    assert results[1].silver_path is None
    # `latest` still advanced to cover the one good snapshot processed.
    assert (silver_root / "latest").resolve() == silver_root / "gtfs_fp2026_20260729"


def test_run_rejects_unknown_mode(bronze_root, monkeypatch):
    monkeypatch.setenv("GTFS_S_RAW_DIR", str(bronze_root))
    with pytest.raises(ValueError, match="unknown transform mode"):
        run(mode="since-yesterday")
