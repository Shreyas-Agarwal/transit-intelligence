# GTFS Static Subset Pipeline

## Inputs

  gtfs_s/raw/gtfs_fp*

## Outputs

  gtfs_s/processed/stops/zurich_stops.parquet
  gtfs_s/processed/trips/zurich_trip_ids.parquet
  gtfs_s/processed/trips/zurich_trips.parquet
  gtfs_s/processed/routes/zurich_routes.parquet

## Execution

```bash
uv run main.py
```
