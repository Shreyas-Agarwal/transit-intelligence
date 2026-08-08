"""Bronze snapshot store layout — mirrors `RawLayout` in
`domains/ingestion/extract/ckan/src/paths.rs`, read-only from this side.
"""

from __future__ import annotations

import os
from pathlib import Path


def bronze_root() -> Path:
    """`<repo_root>/data/bronze/static` by default, overridable with
    `GTFS_S_RAW_DIR` — the same env var and default the Rust `ckan` downloader
    uses (`domains/ingestion/extract/ckan/src/config.rs`), so both halves of
    the pipeline agree on where snapshots live without duplicating the default.
    """
    raw_dir = os.environ.get("GTFS_S_RAW_DIR")
    if raw_dir:
        return Path(raw_dir)
    repo_root = Path(__file__).resolve().parents[5]
    return repo_root / "data" / "bronze" / "static"


def is_snapshot_dir(path: Path) -> bool:
    """Excludes the downloader's reserved entries — `.staging`, `.manifest.json`,
    `.updater.lock`, `latest` — mirroring
    `RawLayout::looks_like_snapshot_dir` in the Rust crate.
    """
    return path.is_dir() and not path.name.startswith(".") and path.name != "latest"
