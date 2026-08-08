from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[3]

GTFS_S_DIR = PROJECT_ROOT / "gtfs_s"

RAW_DIR = GTFS_S_DIR / "raw"
PROCESSED_DIR = GTFS_S_DIR / "processed"

GTFS_DIR = sorted(
    RAW_DIR.glob("gtfs_fp*"),
    reverse=True
)[0]
