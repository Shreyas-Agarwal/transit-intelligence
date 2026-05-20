import time
import logging
import sys
from datetime import datetime, timezone
import duckdb

# Configure structured JSON logging
logging.basicConfig(
    level=logging.INFO,
    format='{"timestamp":"%(asctime)s", "level":"%(levelname)s", "service":"workers", "message":"%(message)s"}',
    datefmt="%Y-%m-%dT%H:%M:%SZ",
    stream=sys.stdout,
)
logger = logging.getLogger("worker")

DB_FILE = "analytics.db"


def init_duckdb():
    """Initializes schema and tables inside the local DuckDB database file."""
    logger.info(f"Initializing DuckDB database file at {DB_FILE}...")
    con = duckdb.connect(DB_FILE)

    # Enable spatial and postgres extensions if needed
    try:
        con.execute("INSTALL postgres; LOAD postgres;")
        logger.info("Successfully loaded DuckDB postgres_scanner extension.")
    except Exception as e:
        logger.warning(f"Could not load postgres extension: {e}")

    # Create stops lookup table (Graph Nodes)
    con.execute("""
        CREATE TABLE IF NOT EXISTS stops (
            stop_id VARCHAR PRIMARY KEY,
            name VARCHAR,
            latitude DOUBLE,
            longitude DOUBLE
        );
    """)

    # Create stop times schedule table
    con.execute("""
        CREATE TABLE IF NOT EXISTS stop_times (
            trip_id VARCHAR,
            stop_id VARCHAR,
            arrival_time VARCHAR,
            departure_time VARCHAR,
            stop_sequence INTEGER,
            PRIMARY KEY (trip_id, stop_id)
        );
    """)

    # Create live updates log table (Event Stream)
    con.execute("""
        CREATE TABLE IF NOT EXISTS vehicle_positions (
            vehicle_id VARCHAR,
            trip_id VARCHAR,
            latitude DOUBLE,
            longitude DOUBLE,
            recorded_at TIMESTAMP,
            delay_seconds INTEGER
        );
    """)

    # Create dynamic transit graph edge weights table
    con.execute("""
        CREATE TABLE IF NOT EXISTS edge_weights (
            source_stop_id VARCHAR,
            target_stop_id VARCHAR,
            trip_id VARCHAR,
            scheduled_duration_seconds INTEGER,
            live_delay_seconds INTEGER,
            weight_seconds DOUBLE,
            last_updated TIMESTAMP,
            PRIMARY KEY (source_stop_id, target_stop_id, trip_id)
        );
    """)

    con.close()
    logger.info("DuckDB schemas initialized successfully.")


def poll_swiss_gtfs_rt():
    """
    Simulates fetching and parsing a binary GTFS-RT protobuf payload.
    In production, this queries the Open Data Swiss HTTP endpoints.
    """
    logger.info("Polling Swiss GTFS-RT protobuf feeds (30s interval)...")

    # Mocking parsed Protobuf entities
    now = datetime.now(timezone.utc)
    mock_updates = [
        {
            "vehicle_id": "ch:vbz:tram:3001",
            "trip_id": "trip:zurich:8001",
            "latitude": 47.3769,
            "longitude": 8.5417,
            "recorded_at": now,
            "delay_seconds": 120,  # 2 minutes delay
        },
        {
            "vehicle_id": "ch:vbz:tram:3002",
            "trip_id": "trip:zurich:8002",
            "latitude": 47.3686,
            "longitude": 8.5391,
            "recorded_at": now,
            "delay_seconds": 45,  # 45 seconds delay
        },
    ]
    return mock_updates


def ingest_updates(updates):
    """Inserts raw updates and updates the dynamic weighted graph edges."""
    if not updates:
        return

    con = duckdb.connect(DB_FILE)

    # Insert updates into the database log
    for update in updates:
        con.execute(
            """
            INSERT INTO vehicle_positions (vehicle_id, trip_id, latitude, longitude, recorded_at, delay_seconds)
            VALUES (?, ?, ?, ?, ?, ?);
        """,
            (
                update["vehicle_id"],
                update["trip_id"],
                update["latitude"],
                update["longitude"],
                update["recorded_at"],
                update["delay_seconds"],
            ),
        )

        # Recalculate dynamic edge weight (scheduled duration + live delay)
        # In a real implementation, we join with stop_times to find target_stop_id
        # Here we upsert edge weight calculations directly for the mock trip
        mock_source = "stop:zurich:central"
        mock_target = "stop:zurich:hauptbahnhof"
        scheduled_dur = 180  # 3 minutes base transit
        live_delay = update["delay_seconds"]
        weight = float(scheduled_dur + live_delay)

        con.execute(
            """
            INSERT OR REPLACE INTO edge_weights (source_stop_id, target_stop_id, trip_id, scheduled_duration_seconds, live_delay_seconds, weight_seconds, last_updated)
            VALUES (?, ?, ?, ?, ?, ?, ?);
        """,
            (
                mock_source,
                mock_target,
                update["trip_id"],
                scheduled_dur,
                live_delay,
                weight,
                datetime.now(timezone.utc),
            ),
        )

        logger.info(
            f"Updated dynamic edge {mock_source} -> {mock_target} for trip {update['trip_id']}. Weight: {weight}s."
        )

    con.close()


def main():
    init_duckdb()

    logger.info("Starting operational analytics loop...")
    while True:
        try:
            updates = poll_swiss_gtfs_rt()
            ingest_updates(updates)
        except Exception as e:
            logger.error(f"Error in ingestion loop: {e}")

        time.sleep(30)


if __name__ == "__main__":
    main()
