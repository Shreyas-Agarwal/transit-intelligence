import polars as pl

from .config import ZurichConfig
from .loader import GtfsLoader

class ZurichSubsetBuilder:
    def __init__(self) -> None:
        self.config = ZurichConfig()

    def build_stops(self) -> pl.DataFrame:
        return (
            GtfsLoader.stops()
            .filter(
                pl.col("stop_name")
                .str.starts_with(
                    self.config.stop_prefix
                )
            )
        )
    
    def build_trip_ids(
        self,
        stops: pl.DataFrame
    ) -> pl.DataFrame:
        
        return (
            GtfsLoader.stop_times()
            .join(
                stops.lazy().select("stop_id"),
                on="stop_id",
                how="semi"
            )
            .select("trip_id")
            .unique()
            .collect()
        )

    def build_trips(
        self,
        trip_ids: pl.DataFrame
    ) -> pl.DataFrame:
        
        return (
            GtfsLoader.trips()
            .join(
                trip_ids.lazy(),
                on="trip_id",
                how="semi"
            )
            .collect()
        )

    def build_routes(
        self,
        trips: pl.DataFrame
    ) -> pl.DataFrame:
        
        return (
            GtfsLoader.routes()
            .join(
                trips.lazy()
                .select("route_id")
                .unique(),
                on="route_id",
                how="semi"
            )
            .collect()
        )