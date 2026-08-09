"""The one transformation pipeline, run identically regardless of which
`snapshot_iterator` (see `snapshots.py`) supplied the snapshot.

`transform_snapshot` has no knowledge of execution modes, loading, SCD Type 2
persistence, PostgreSQL, or any other downstream consumer. Its sole
responsibility is to turn one Bronze snapshot into one transformed (Silver)
snapshot:

1. Load every Parquet table the Bronze snapshot actually contains.
2. Validate them (Tier 2 checks — `validate.py`). A failing snapshot stops
   here: no Silver output is written for it, mirroring the Bronze downloader's
   own "never publish something broken" invariant.
3. Derive the Zurich operational subset (ADR 0011 — `subset.py`) and publish
   it to `data/silver/static/<version>/`, advancing `latest` (`silver_paths.py`).
4. Derive the canonical transit graph (`graph.py`) from that *same* version's
   tables and publish it to `data/silver/graph/<version>/`, advancing its own
   `latest` — never from an independently-resolved `latest`, since the
   snapshot already fixes which version this is. Graph construction is
   deliberately isolated from step 3: a graph failure never touches the
   already-published static Silver output, and never leaves `data/silver/graph/latest`
   pointing at a partially-written version.
"""

from __future__ import annotations

import logging
from dataclasses import dataclass
from pathlib import Path

import polars as pl

from .graph import TransitGraph, build_transit_graph
from .silver_paths import SilverLayout, graph_root
from .snapshots import Snapshot
from .subset import build_zurich_subset
from .validate import ValidationReport, validate_snapshot

logger = logging.getLogger(__name__)


@dataclass
class TransformResult:
    snapshot: Snapshot
    validation: ValidationReport
    # None when validation failed — no Silver output was written for this snapshot.
    silver_path: Path | None
    artifact_row_counts: dict[str, int] | None
    # None when validation failed, or when graph construction/publishing itself
    # failed — the static Silver output above is unaffected either way.
    graph_path: Path | None = None
    graph_row_counts: dict[str, int] | None = None


def _load_tables(snapshot: Snapshot) -> dict[str, pl.DataFrame]:
    """Loads every Parquet file present in the snapshot, keyed by GTFS file
    stem (`"stops"`, `"stop_times"`, `"agency"`, ...) — mirrors the Rust
    downloader's own "convert every *.txt present" symmetry, so this doesn't
    need updating whenever the GTFS spec (or this feed) grows an optional file.
    """
    return {path.stem: pl.read_parquet(path) for path in sorted(snapshot.path.glob("*.parquet"))}


def _write_silver_snapshot(snapshot: Snapshot, artifacts: dict[str, pl.DataFrame]) -> Path:
    layout = SilverLayout()
    staging_dir = layout.staging_dir(snapshot.version)
    staging_dir.mkdir(parents=True, exist_ok=True)

    for name, df in artifacts.items():
        df.write_parquet(staging_dir / f"{name}.parquet")

    final_dir = layout.publish(staging_dir, snapshot.version)
    layout.advance_latest_if_newer(snapshot.version)
    return final_dir


def _write_graph_snapshot(snapshot: Snapshot, graph: TransitGraph) -> Path:
    layout = SilverLayout(graph_root())
    staging_dir = layout.staging_dir(snapshot.version)
    staging_dir.mkdir(parents=True, exist_ok=True)

    graph.nodes.write_parquet(staging_dir / "nodes.parquet")
    graph.edges.write_parquet(staging_dir / "edges.parquet")

    # Only reached once both files above are written — `publish` never sees a
    # half-written staging dir, so `latest` never advances to one either.
    final_dir = layout.publish(staging_dir, snapshot.version)
    layout.advance_latest_if_newer(snapshot.version)
    return final_dir


def transform_snapshot(snapshot: Snapshot) -> TransformResult:
    """Transforms one Bronze snapshot into one transformed snapshot."""
    tables = _load_tables(snapshot)
    report = validate_snapshot(snapshot.version, tables)

    if not report.passed:
        return TransformResult(
            snapshot=snapshot, validation=report, silver_path=None, artifact_row_counts=None
        )

    subset = build_zurich_subset(tables)
    artifacts = subset.artifacts()
    silver_path = _write_silver_snapshot(snapshot, artifacts)

    graph_path: Path | None = None
    graph_row_counts: dict[str, int] | None = None
    try:
        graph = build_transit_graph(tables, subset.stops)
        graph_path = _write_graph_snapshot(snapshot, graph)
        graph_row_counts = {"nodes": graph.nodes.height, "edges": graph.edges.height}
    except Exception:
        logger.exception(
            "graph construction failed for snapshot %s; static Silver output at %s is unaffected",
            snapshot.version,
            silver_path,
        )

    return TransformResult(
        snapshot=snapshot,
        validation=report,
        silver_path=silver_path,
        artifact_row_counts={name: df.height for name, df in artifacts.items()},
        graph_path=graph_path,
        graph_row_counts=graph_row_counts,
    )
