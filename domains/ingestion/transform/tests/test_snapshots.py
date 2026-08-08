from __future__ import annotations

from pathlib import Path

from ingestion.transform.snapshots import iter_latest, iter_replay

from .conftest import write_snapshot


def _make_bronze_tree(root: Path, good_tables) -> None:
    write_snapshot(root, "gtfs_fp2026_20260722", good_tables)
    write_snapshot(root, "gtfs_fp2026_20260729", good_tables)
    write_snapshot(root, "gtfs_fp2026_20260805", good_tables)
    (root / "latest").symlink_to("gtfs_fp2026_20260805")
    # Reserved entries an iterator must ignore.
    (root / ".manifest.json").write_text("{}")
    (root / ".staging").mkdir()


def test_iter_latest_yields_only_the_symlink_target(bronze_root, good_tables):
    _make_bronze_tree(bronze_root, good_tables)

    snapshots = list(iter_latest(bronze_root))

    assert [s.version for s in snapshots] == ["gtfs_fp2026_20260805"]


def test_iter_replay_yields_every_snapshot_oldest_first(bronze_root, good_tables):
    _make_bronze_tree(bronze_root, good_tables)

    snapshots = list(iter_replay(bronze_root))

    assert [s.version for s in snapshots] == [
        "gtfs_fp2026_20260722",
        "gtfs_fp2026_20260729",
        "gtfs_fp2026_20260805",
    ]


def test_iter_replay_ignores_reserved_entries(bronze_root, good_tables):
    _make_bronze_tree(bronze_root, good_tables)

    versions = {s.version for s in iter_replay(bronze_root)}

    assert "latest" not in versions
    assert ".manifest.json" not in versions
    assert ".staging" not in versions
