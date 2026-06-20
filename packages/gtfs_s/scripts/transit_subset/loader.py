import polars as pl
from .paths import GTFS_DIR

class GtfsLoader:
    @staticmethod
    def stops() -> pl.DataFrame:
        return pl.read_csv(
            GTFS_DIR / "stops.txt"
        )

    @staticmethod
    def trips() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "trips.txt"
        )

    @staticmethod
    def routes() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "routes.txt"
        )

    @staticmethod
    def stop_times() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "stop_times.txt"
        )