"""Design doc Tier 2 content-level validation
(`docs/design/gtfs-static-auto-downloader.md` §6), run with Polars directly —
no DuckDB, no SQLMesh. Checks the archive-level (Tier 1, Rust) checks
deliberately don't: required columns, row-count sanity, referential
integrity.
"""

from __future__ import annotations

from dataclasses import dataclass, field

import polars as pl

REQUIRED_COLUMNS: dict[str, tuple[str, ...]] = {
    "stops": ("stop_id", "stop_name", "stop_lat", "stop_lon"),
    "routes": ("route_id",),
    "trips": ("trip_id", "route_id"),
    "stop_times": ("trip_id", "stop_id", "stop_sequence"),
}

# ADR 0011's documented order-of-magnitude row counts (nationwide feed),
# bounded to roughly 10x either side — "a sane order of magnitude", not just
# `> 0`.
ROW_COUNT_RANGE: dict[str, tuple[int, int]] = {
    "stops": (10_000, 1_000_000),
    "routes": (500, 50_000),
    "trips": (100_000, 10_000_000),
    "stop_times": (1_000_000, 100_000_000),
}

# (child_table, child_column, parent_table, parent_column)
REFERENTIAL_INTEGRITY: tuple[tuple[str, str, str, str], ...] = (
    ("trips", "route_id", "routes", "route_id"),
    ("stop_times", "trip_id", "trips", "trip_id"),
    ("stop_times", "stop_id", "stops", "stop_id"),
)


@dataclass
class CheckFailure:
    table: str
    check: str
    detail: str

    def __str__(self) -> str:
        return f"{self.table}.{self.check}: {self.detail}"


@dataclass
class ValidationReport:
    snapshot_version: str
    failures: list[CheckFailure] = field(default_factory=list)

    @property
    def passed(self) -> bool:
        return not self.failures

    def add(self, table: str, check: str, detail: str) -> None:
        self.failures.append(CheckFailure(table, check, detail))


class ValidationFailed(Exception):
    def __init__(self, report: ValidationReport) -> None:
        self.report = report
        details = "\n".join(f"  - {f}" for f in report.failures)
        super().__init__(f"snapshot {report.snapshot_version} failed validation:\n{details}")


def _check_required_columns(table: str, df: pl.DataFrame, report: ValidationReport) -> None:
    missing = [c for c in REQUIRED_COLUMNS.get(table, ()) if c not in df.columns]
    if missing:
        report.add(table, "required_columns", f"missing column(s): {missing}")


def _check_not_null(table: str, df: pl.DataFrame, report: ValidationReport) -> None:
    for col in REQUIRED_COLUMNS.get(table, ()):
        if col not in df.columns:
            continue  # already reported by _check_required_columns
        null_count = df[col].null_count()
        if null_count:
            report.add(table, "not_null", f"{col} has {null_count} null value(s)")


def _check_row_count_range(table: str, df: pl.DataFrame, report: ValidationReport) -> None:
    bounds = ROW_COUNT_RANGE.get(table)
    if bounds is None:
        return
    min_rows, max_rows = bounds
    if not (min_rows <= df.height <= max_rows):
        report.add(
            table,
            "row_count_range",
            f"{df.height} rows outside expected [{min_rows}, {max_rows}] "
            "(ADR 0011 order-of-magnitude bound)",
        )


def _check_referential_integrity(tables: dict[str, pl.DataFrame], report: ValidationReport) -> None:
    for child_table, child_col, parent_table, parent_col in REFERENTIAL_INTEGRITY:
        child = tables.get(child_table)
        parent = tables.get(parent_table)
        if child is None or parent is None:
            continue
        if child_col not in child.columns or parent_col not in parent.columns:
            continue  # already reported by _check_required_columns

        orphans = (
            child.select(pl.col(child_col))
            .drop_nulls()
            .join(
                parent.select(pl.col(parent_col).alias(child_col)).unique(),
                on=child_col,
                how="anti",
            )
        )
        if orphans.height:
            report.add(
                child_table,
                "referential_integrity",
                f"{orphans.height} {child_col} value(s) not found in {parent_table}.{parent_col}",
            )


def validate_snapshot(snapshot_version: str, tables: dict[str, pl.DataFrame]) -> ValidationReport:
    """Runs every Tier 2 check against the given tables (keyed by GTFS file
    stem, e.g. `"stops"`, `"stop_times"`) and returns a report — never raises;
    callers decide what a failed report means for their run (see `pipeline.py`
    and `run.py`).
    """
    report = ValidationReport(snapshot_version=snapshot_version)

    for table, df in tables.items():
        _check_required_columns(table, df, report)
        _check_not_null(table, df, report)
        _check_row_count_range(table, df, report)

    _check_referential_integrity(tables, report)

    return report
