from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[3]

RAW_DIR = PROJECT_ROOT/ "data" / "raw"
PROCESSED_DIR = PROJECT_ROOT/ "data" / "processed"

GTFS_DIR = sorted(
    RAW_DIR.glob("gtfs_fp*"),
    reverse=True
)[0]