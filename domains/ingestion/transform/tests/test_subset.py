from __future__ import annotations

import polars as pl

from ingestion.transform.subset import build_zurich_subset

# Small, hand-crafted universe: 2 Zurich stops, 2 non-Zurich stops.
STOPS = pl.DataFrame(
    {
        "stop_id": ["Z1", "Z2", "X1", "X2"],
        "stop_name": ["Zürich HB", "Zürich Oerlikon", "Bern", "Basel"],
    }
)
ROUTES = pl.DataFrame({"route_id": ["R1", "R2"], "agency_id": ["A1", "A1"]})
# T_INTERNAL only ever touches Zurich stops; T_CROSSING touches one of each;
# T_IGNORED never touches a Zurich stop at all, so it shouldn't appear anywhere
# in the Zurich universe.
TRIPS = pl.DataFrame(
    {
        "trip_id": ["T_INTERNAL", "T_CROSSING", "T_IGNORED"],
        "route_id": ["R1", "R2", "R1"],
        "service_id": ["SVC1", "SVC1", "SVC1"],
    }
)
STOP_TIMES = pl.DataFrame(
    {
        "trip_id": ["T_INTERNAL", "T_INTERNAL", "T_CROSSING", "T_CROSSING", "T_IGNORED"],
        "stop_id": ["Z1", "Z2", "Z1", "X1", "X2"],
        "stop_sequence": [1, 2, 1, 2, 1],
    }
)


def _tables(**extra: pl.DataFrame) -> dict[str, pl.DataFrame]:
    return {
        "stops": STOPS,
        "routes": ROUTES,
        "trips": TRIPS,
        "stop_times": STOP_TIMES,
        **extra,
    }


def test_stops_keeps_every_row_and_classifies_stop_type():
    subset = build_zurich_subset(_tables())
    assert sorted(subset.stops["stop_id"]) == ["X1", "X2", "Z1", "Z2"]
    stop_types = dict(zip(subset.stops["stop_id"], subset.stops["stop_type"], strict=True))
    assert stop_types == {
        "Z1": "internal",
        "Z2": "internal",
        # X1 is visited by T_CROSSING, which also visits an internal stop.
        "X1": "boundary",
        # X2 is only ever visited by T_IGNORED, which touches no internal stop.
        "X2": "external",
    }


def test_internal_stops_matches_the_old_zurich_prefix_filter():
    subset = build_zurich_subset(_tables())
    assert sorted(subset.internal_stops["stop_id"]) == ["Z1", "Z2"]
    assert "stop_type" not in subset.internal_stops.columns


def test_trip_universe_excludes_trips_with_no_zurich_stop():
    subset = build_zurich_subset(_tables())
    assert sorted(subset.trips["trip_id"]) == ["T_CROSSING", "T_INTERNAL"]
    assert "T_IGNORED" not in subset.trip_ids["trip_id"].to_list()


def test_route_universe_derived_from_zurich_trips():
    subset = build_zurich_subset(_tables())
    # R1 is reachable via T_INTERNAL (and T_IGNORED, which doesn't count) —
    # both R1 and R2 belong to the Zurich universe because each is used by at
    # least one Zurich trip.
    assert sorted(subset.routes["route_id"]) == ["R1", "R2"]


def test_internal_vs_crossing_classification():
    subset = build_zurich_subset(_tables())
    assert subset.internal_trips["trip_id"].to_list() == ["T_INTERNAL"]
    assert subset.crossing_trips["trip_id"].to_list() == ["T_CROSSING"]


def test_route_classification_matches_its_trips():
    subset = build_zurich_subset(_tables())
    # R1 carries only T_INTERNAL (within the Zurich universe) -> internal.
    # R2 carries only T_CROSSING -> crossing. Neither is mixed here.
    assert subset.internal_routes["route_id"].to_list() == ["R1"]
    assert subset.crossing_routes["route_id"].to_list() == ["R2"]
    assert subset.mixed_routes.height == 0


def test_optional_tables_absent_yield_none_and_are_skipped_by_artifacts():
    subset = build_zurich_subset(_tables())
    assert subset.calendar is None
    assert subset.calendar_dates is None
    assert subset.agencies is None
    assert subset.frequencies is None
    assert "calendar" not in subset.artifacts()
    assert "agencies" not in subset.artifacts()


def test_optional_tables_present_are_subset_and_included_in_artifacts():
    agency = pl.DataFrame({"agency_id": ["A1", "A2"], "agency_name": ["Agency 1", "Agency 2"]})
    calendar = pl.DataFrame({"service_id": ["SVC1", "SVC_UNUSED"], "monday": [1, 1]})

    subset = build_zurich_subset(_tables(agency=agency, calendar=calendar))

    assert subset.agencies is not None
    assert subset.agencies["agency_id"].to_list() == ["A1"]
    assert subset.calendar is not None
    assert subset.calendar["service_id"].to_list() == ["SVC1"]
    assert "agencies" in subset.artifacts()
    assert "calendar" in subset.artifacts()
