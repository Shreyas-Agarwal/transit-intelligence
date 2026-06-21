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

    @staticmethod
    def calendar() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "calendar.txt"
        )

    @staticmethod
    def calendar_dates() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "calendar_dates.txt"
        )

    @staticmethod
    def agencies() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "agency.txt"
        )

    @staticmethod
    def frequencies() -> pl.LazyFrame:
        return pl.scan_csv(
            GTFS_DIR / "frequencies.txt"
        )