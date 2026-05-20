import os
import pytest
import duckdb
from datetime import datetime, timezone
from main import init_duckdb, poll_swiss_gtfs_rt, ingest_updates

# Override DB_FILE for test isolation
TEST_DB_FILE = "test_analytics.db"


@pytest.fixture(autouse=True)
def setup_test_db():
    """Sets up a clean test database and tears it down after tests."""
    import main

    original_db = main.DB_FILE
    main.DB_FILE = TEST_DB_FILE

    # Remove test DB file if it already exists
    if os.path.exists(TEST_DB_FILE):
        os.remove(TEST_DB_FILE)

    yield

    # Cleanup
    if os.path.exists(TEST_DB_FILE):
        try:
            os.remove(TEST_DB_FILE)
        except Exception:
            pass
    main.DB_FILE = original_db


def test_init_duckdb():
    """Verifies that init_duckdb creates all required tables with expected columns."""
    init_duckdb()

    con = duckdb.connect(TEST_DB_FILE)

    # Check stops table
    stops_columns = con.execute("DESCRIBE stops;").fetchall()
    columns_dict = {col[0]: col[1] for col in stops_columns}
    assert "stop_id" in columns_dict
    assert "name" in columns_dict
    assert "latitude" in columns_dict
    assert "longitude" in columns_dict

    # Check vehicle_positions table
    pos_columns = con.execute("DESCRIBE vehicle_positions;").fetchall()
    pos_dict = {col[0]: col[1] for col in pos_columns}
    assert "vehicle_id" in pos_dict
    assert "delay_seconds" in pos_dict

    # Check edge_weights table
    edge_columns = con.execute("DESCRIBE edge_weights;").fetchall()
    edge_dict = {col[0]: col[1] for col in edge_columns}
    assert "source_stop_id" in edge_dict
    assert "target_stop_id" in edge_dict
    assert "weight_seconds" in edge_dict

    con.close()


def test_poll_swiss_gtfs_rt():
    """Verifies that mock Swiss GTFS polling returns formatted entities."""
    updates = poll_swiss_gtfs_rt()
    assert isinstance(updates, list)
    assert len(updates) > 0
    assert "vehicle_id" in updates[0]
    assert "trip_id" in updates[0]
    assert "delay_seconds" in updates[0]


def test_ingest_updates():
    """Verifies that ingesting telemetry computes and inserts edge weights."""
    init_duckdb()

    now = datetime.now(timezone.utc)
    mock_updates = [
        {
            "vehicle_id": "ch:vbz:tram:test",
            "trip_id": "trip:test:1",
            "latitude": 47.3769,
            "longitude": 8.5417,
            "recorded_at": now,
            "delay_seconds": 120,
        }
    ]

    ingest_updates(mock_updates)

    con = duckdb.connect(TEST_DB_FILE)

    # Assert telemetry is logged
    logs = con.execute("SELECT * FROM vehicle_positions;").fetchall()
    assert len(logs) == 1
    assert logs[0][0] == "ch:vbz:tram:test"
    assert logs[0][5] == 120  # delay_seconds

    # Assert dynamic edge weights are calculated and stored
    weights = con.execute("SELECT * FROM edge_weights;").fetchall()
    assert len(weights) == 1
    assert weights[0][0] == "stop:zurich:central"
    assert weights[0][1] == "stop:zurich:hauptbahnhof"
    assert weights[0][3] == 180  # scheduled duration
    assert weights[0][4] == 120  # live delay
    assert weights[0][5] == 300.0  # weight_seconds = 180 + 120

    con.close()
