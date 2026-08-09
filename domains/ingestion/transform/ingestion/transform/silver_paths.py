"""Silver snapshot store layout — same conventions as Bronze's (`paths.py`,
which itself mirrors `RawLayout` in domains/ingestion/extract/ckan/src/paths.rs):
one directory per snapshot version, a `latest` symlink that only ever advances
forward. Rooted at `data/silver/static` instead of `data/bronze/static`, and
writable from this side since this stage produces it rather than just reading it.

`SilverLayout` is deliberately root-agnostic — it takes whatever root it's
given — so the graph output (`graph_root()`, `data/silver/graph`) reuses the
exact same versioned-snapshot + `latest` machinery as the static output
instead of a parallel implementation. Same layout, different root.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path


def silver_root() -> Path:
    """`<repo_root>/data/silver/static` by default, overridable with
    `GTFS_S_SILVER_DIR` — analogous to Bronze's `GTFS_S_RAW_DIR`.
    """
    silver_dir = os.environ.get("GTFS_S_SILVER_DIR")
    if silver_dir:
        return Path(silver_dir)
    repo_root = Path(__file__).resolve().parents[5]
    return repo_root / "data" / "silver" / "static"


def graph_root() -> Path:
    """`<repo_root>/data/silver/graph` by default, overridable with
    `GTFS_S_SILVER_GRAPH_DIR` — same idea as `silver_root()`, just the graph
    output's own root, kept separate from `data/silver/static` (ADR-free v1
    graph output, alongside the static Silver layer rather than inside it).
    """
    graph_dir = os.environ.get("GTFS_S_SILVER_GRAPH_DIR")
    if graph_dir:
        return Path(graph_dir)
    repo_root = Path(__file__).resolve().parents[5]
    return repo_root / "data" / "silver" / "graph"


class SilverLayout:
    def __init__(self, root: Path | None = None) -> None:
        self.root = root or silver_root()

    def staging_dir(self, version: str) -> Path:
        return self.root / ".staging" / version

    def final_dir(self, version: str) -> Path:
        return self.root / version

    def latest_symlink_path(self) -> Path:
        return self.root / "latest"

    def current_latest_version(self) -> str | None:
        link = self.latest_symlink_path()
        if not link.is_symlink():
            return None
        return link.resolve().name

    def publish(self, staging_dir: Path, version: str) -> Path:
        """Atomically moves a fully-written staging directory to its final
        snapshot path — a same-filesystem rename, so there's no intermediate
        state a concurrent reader could observe (mirrors the Rust downloader's
        publish step, design doc §5 step 7).
        """
        self.root.mkdir(parents=True, exist_ok=True)
        final_dir = self.final_dir(version)
        if final_dir.exists():
            shutil.rmtree(final_dir)
        os.replace(staging_dir, final_dir)
        return final_dir

    def advance_latest_if_newer(self, version: str) -> None:
        """Repoints `latest` at `version` via a temp-symlink-then-rename swap
        (the `ln -sfn` equivalent) — atomic, so readers always see either the
        old or the new target, never a broken/missing link.

        Only advances forward: if `version` sorts before the current target,
        this is a no-op. Mirrors the Bronze downloader's own invariant
        (design doc §7) — replaying older history should never regress what
        `latest` points at.
        """
        current = self.current_latest_version()
        if current is not None and version <= current:
            return

        self.root.mkdir(parents=True, exist_ok=True)
        tmp_path = self.root / f".latest.tmp.{os.getpid()}"
        if tmp_path.is_symlink() or tmp_path.exists():
            tmp_path.unlink()
        tmp_path.symlink_to(version)
        os.replace(tmp_path, self.latest_symlink_path())
