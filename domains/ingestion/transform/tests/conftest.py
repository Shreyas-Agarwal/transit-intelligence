from __future__ import annotations

from pathlib import Path

import polars as pl
import pytest

N_STOPS = 20_000
N_ROUTES = 1_000
N_TRIPS = 200_000
N_STOP_TIMES = 2_000_000
N_SERVICES = 50
N_AGENCIES = 10

# Every 20th stop is a Zurich stop (ADR 0011's `stop_name` filter), so the
# subset builder has a non-trivial (but still bounded) universe to derive from.
GOOD_STOPS = pl.DataFrame(
    {
        "stop_id": [f"S{i}" for i in range(N_STOPS)],
        "stop_name": [f"Zürich Stop {i}" if i % 20 == 0 else f"Stop {i}" for i in range(N_STOPS)],
        "stop_lat": [46.5] * N_STOPS,
        "stop_lon": [7.5] * N_STOPS,
    }
)
GOOD_ROUTES = pl.DataFrame(
    {
        "route_id": [f"R{i}" for i in range(N_ROUTES)],
        "agency_id": [f"A{i % N_AGENCIES}" for i in range(N_ROUTES)],
    }
)
GOOD_TRIPS = pl.DataFrame(
    {
        "trip_id": [f"T{i}" for i in range(N_TRIPS)],
        "route_id": [f"R{i % N_ROUTES}" for i in range(N_TRIPS)],
        "service_id": [f"SVC{i % N_SERVICES}" for i in range(N_TRIPS)],
    }
)
# 10 stop_times per trip (2,000,000 / 200,000). `stop_id` is spread with a
# stride coprime to N_STOPS so a trip's 10 stops land on varied stop indices
# (a mix of Zurich and non-Zurich) rather than colliding on a single repeated
# stop — otherwise every trip would trivially classify as 100% one or the
# other and the internal/crossing split would never be exercised.
GOOD_STOP_TIMES = pl.DataFrame(
    {
        "trip_id": [f"T{i // 10}" for i in range(N_STOP_TIMES)],
        "stop_id": [f"S{((i // 10) * 7 + i % 10) % N_STOPS}" for i in range(N_STOP_TIMES)],
        "stop_sequence": [i % 10 for i in range(N_STOP_TIMES)],
    }
)
GOOD_AGENCY = pl.DataFrame(
    {
        "agency_id": [f"A{i}" for i in range(N_AGENCIES)],
        "agency_name": [f"Agency {i}" for i in range(N_AGENCIES)],
    }
)
GOOD_CALENDAR = pl.DataFrame(
    {
        "service_id": [f"SVC{i}" for i in range(N_SERVICES)],
        "monday": [1] * N_SERVICES,
    }
)
GOOD_CALENDAR_DATES = pl.DataFrame(
    {
        "service_id": [f"SVC{i % N_SERVICES}" for i in range(N_SERVICES)],
        "date": ["20260805"] * N_SERVICES,
        "exception_type": [1] * N_SERVICES,
    }
)
GOOD_FREQUENCIES = pl.DataFrame(
    {
        "trip_id": [f"T{i}" for i in range(0, N_TRIPS, N_TRIPS // 10)],
        "headway_secs": [600] * 10,
    }
)


def write_snapshot(root: Path, version: str, tables: dict[str, pl.DataFrame]) -> Path:
    snapshot_dir = root / version
    snapshot_dir.mkdir(parents=True)
    for name, df in tables.items():
        df.write_parquet(snapshot_dir / f"{name}.parquet")
    return snapshot_dir


@pytest.fixture
def good_tables() -> dict[str, pl.DataFrame]:
    """The required tables only — enough to exercise validation and the core
    subset derivation. `optional_tables` below adds the rest.
    """
    return {
        "stops": GOOD_STOPS,
        "routes": GOOD_ROUTES,
        "trips": GOOD_TRIPS,
        "stop_times": GOOD_STOP_TIMES,
    }


@pytest.fixture
def optional_tables() -> dict[str, pl.DataFrame]:
    """The optional GTFS tables the subset builder treats as present-if-given:
    `agency`, `calendar`, `calendar_dates`, `frequencies`."""
    return {
        "agency": GOOD_AGENCY,
        "calendar": GOOD_CALENDAR,
        "calendar_dates": GOOD_CALENDAR_DATES,
        "frequencies": GOOD_FREQUENCIES,
    }


@pytest.fixture
def bronze_root(tmp_path: Path) -> Path:
    return tmp_path / "bronze" / "static"


@pytest.fixture
def silver_root(tmp_path: Path) -> Path:
    return tmp_path / "silver" / "static"


@pytest.fixture
def graph_root(tmp_path: Path) -> Path:
    return tmp_path / "silver" / "graph"
