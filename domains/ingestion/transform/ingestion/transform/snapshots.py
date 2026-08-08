"""Snapshot discovery: *which* Bronze snapshots a run should transform.

Deliberately separate from `pipeline.transform_snapshot` (*how* one snapshot
gets transformed) — see that module's docstring. Adding a new execution
policy (a specific version, a version range, since-version, ...) means adding
a new iterator here; `transform_snapshot` never changes.
"""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

from .paths import bronze_root, is_snapshot_dir


@dataclass(frozen=True)
class Snapshot:
    """One Bronze snapshot: its version identifier and the directory of
    Parquet files (`stops.parquet`, `trips.parquet`, ...) it contains.
    """

    version: str
    path: Path


def iter_latest(root: Path | None = None) -> Iterator[Snapshot]:
    """Normal operation (design doc §7): the single snapshot `latest` points
    at, resolved through the symlink the Rust downloader maintains.
    """
    base = root or bronze_root()
    target = (base / "latest").resolve(strict=True)
    yield Snapshot(version=target.name, path=target)


def iter_replay(root: Path | None = None) -> Iterator[Snapshot]:
    """Historical reconstruction: every retained snapshot, oldest first.

    Every downloaded snapshot is kept indefinitely (design doc §8), so this
    walks the full history rather than just what `latest` points at.
    """
    base = root or bronze_root()
    versions = sorted(
        (p for p in base.iterdir() if is_snapshot_dir(p)),
        key=lambda p: p.name,
    )
    for path in versions:
        yield Snapshot(version=path.name, path=path)


SNAPSHOT_ITERATORS = {
    "latest": iter_latest,
    "replay": iter_replay,
}
