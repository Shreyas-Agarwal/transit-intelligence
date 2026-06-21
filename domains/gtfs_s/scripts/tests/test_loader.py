from unittest.mock import patch, MagicMock
from transit_subset.loader import GtfsLoader

@patch("transit_subset.loader.pl")
def test_loader_stops(mock_pl):
    mock_pl.read_csv.return_value = "mock_stops_df"
    result = GtfsLoader.stops()
    assert result == "mock_stops_df"
    mock_pl.read_csv.assert_called_once()
    assert "stops.txt" in str(mock_pl.read_csv.call_args[0][0])

@patch("transit_subset.loader.pl")
def test_loader_trips(mock_pl):
    mock_pl.scan_csv.return_value = "mock_trips_lazy"
    result = GtfsLoader.trips()
    assert result == "mock_trips_lazy"
    mock_pl.scan_csv.assert_called_once()
    assert "trips.txt" in str(mock_pl.scan_csv.call_args[0][0])

@patch("transit_subset.loader.pl")
def test_loader_routes(mock_pl):
    mock_pl.scan_csv.return_value = "mock_routes_lazy"
    result = GtfsLoader.routes()
    assert result == "mock_routes_lazy"
    mock_pl.scan_csv.assert_called_once()
    assert "routes.txt" in str(mock_pl.scan_csv.call_args[0][0])

@patch("transit_subset.loader.pl")
def test_loader_stop_times(mock_pl):
    mock_pl.scan_csv.return_value = "mock_stop_times_lazy"
    result = GtfsLoader.stop_times()
    assert result == "mock_stop_times_lazy"
    mock_pl.scan_csv.assert_called_once()
    assert "stop_times.txt" in str(mock_pl.scan_csv.call_args[0][0])
