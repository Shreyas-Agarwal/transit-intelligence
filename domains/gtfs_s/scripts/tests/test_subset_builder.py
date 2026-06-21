from unittest.mock import patch

import polars as pl
from transit_subset.subset_builder import ZurichSubsetBuilder


@patch("transit_subset.subset_builder.GtfsLoader")
def test_build_stops(mock_loader, mock_stops):
    mock_loader.stops.return_value = mock_stops

    builder = ZurichSubsetBuilder()
    # builder.config.stop_prefix is "Zürich" by default
    result = builder.build_stops()

    assert result.height == 2
    assert "s3" not in result.get_column("stop_id").to_list()


@patch("transit_subset.subset_builder.GtfsLoader")
def test_build_trip_ids(mock_loader, mock_stop_times):
    mock_loader.stop_times.return_value = mock_stop_times

    builder = ZurichSubsetBuilder()

    # Mock filtered stops
    stops = pl.DataFrame({"stop_id": ["s1"]})

    result = builder.build_trip_ids(stops)

    assert result.height == 2
    assert "t1" in result.get_column("trip_id").to_list()
    assert "t3" in result.get_column("trip_id").to_list()
    assert "t2" not in result.get_column("trip_id").to_list()


@patch("transit_subset.subset_builder.GtfsLoader")
def test_build_trips(mock_loader, mock_trips):
    mock_loader.trips.return_value = mock_trips

    builder = ZurichSubsetBuilder()

    # Mock trip_ids
    trip_ids = pl.DataFrame({"trip_id": ["t1"]})

    result = builder.build_trips(trip_ids)

    assert result.height == 1
    assert result.get_column("route_id")[0] == "r1"


@patch("transit_subset.subset_builder.GtfsLoader")
def test_build_routes(mock_loader, mock_routes):
    mock_loader.routes.return_value = mock_routes

    builder = ZurichSubsetBuilder()

    # Mock filtered trips
    trips = pl.DataFrame({"route_id": ["r2"]})

    result = builder.build_routes(trips)

    assert result.height == 1
    assert result.get_column("route_id")[0] == "r2"
