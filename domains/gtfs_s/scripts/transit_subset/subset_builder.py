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

    def build_classified_trips(
        self,
        zurich_stops: pl.DataFrame,
        zurich_trips: pl.DataFrame
    ) -> tuple[pl.DataFrame, pl.DataFrame]:
        
        stop_times = GtfsLoader.stop_times()
        
        # Only process stop times for our known Zurich trips
        # to avoid scanning everything unnecessarily
        relevant_stop_times = stop_times.join(
            zurich_trips.lazy().select("trip_id"),
            on="trip_id",
            how="semi"
        )
        
        trip_stop_counts = (
            relevant_stop_times
            .group_by("trip_id")
            .agg(pl.len().alias("total_stops"))
        )

        zurich_stop_counts = (
            relevant_stop_times
            .join(
                zurich_stops.lazy().select("stop_id"),
                on="stop_id",
                how="semi"
            )
            .group_by("trip_id")
            .agg(pl.len().alias("zurich_stops"))
        )

        trip_classification = (
            trip_stop_counts
            .join(
                zurich_stop_counts,
                on="trip_id",
                how="inner"
            )
            .with_columns(
                pl.when(pl.col("total_stops") == pl.col("zurich_stops"))
                .then(pl.lit("internal"))
                .otherwise(pl.lit("crossing"))
                .alias("trip_type")
            )
        )
        
        classified_trips = (
            zurich_trips.lazy()
            .join(trip_classification, on="trip_id", how="inner")
            .collect()
        )
        
        internal_trips = classified_trips.filter(
            pl.col("trip_type") == "internal"
        ).drop("trip_type", "total_stops", "zurich_stops")
        
        crossing_trips = classified_trips.filter(
            pl.col("trip_type") == "crossing"
        ).drop("trip_type", "total_stops", "zurich_stops")
        
        return internal_trips, crossing_trips

    def build_classified_routes(
        self,
        zurich_routes: pl.DataFrame,
        internal_trips: pl.DataFrame,
        crossing_trips: pl.DataFrame
    ) -> tuple[pl.DataFrame, pl.DataFrame, pl.DataFrame]:
        
        internal = internal_trips.lazy().with_columns(pl.lit("internal").alias("trip_type"))
        crossing = crossing_trips.lazy().with_columns(pl.lit("crossing").alias("trip_type"))
        
        classified_trips = pl.concat([internal, crossing])
        
        route_classification = (
            classified_trips
            .group_by("route_id")
            .agg(
                pl.col("trip_type").unique()
            )
            .with_columns(
                pl.when(pl.col("trip_type").list.len() == 1)
                .then(pl.col("trip_type").list.first())
                .otherwise(pl.lit("mixed"))
                .alias("classification")
            )
        )
        
        classified_routes = (
            zurich_routes.lazy()
            .join(route_classification, on="route_id", how="inner")
            .collect()
        )
        
        internal_routes = classified_routes.filter(
            pl.col("classification") == "internal"
        ).drop("classification", "trip_type")
        
        crossing_routes = classified_routes.filter(
            pl.col("classification") == "crossing"
        ).drop("classification", "trip_type")
        
        mixed_routes = classified_routes.filter(
            pl.col("classification") == "mixed"
        ).drop("classification", "trip_type")
        
        return internal_routes, crossing_routes, mixed_routes

    def build_stop_times(
        self,
        trip_ids: pl.DataFrame
    ) -> pl.DataFrame:
        return (
            GtfsLoader.stop_times()
            .join(
                trip_ids.lazy().select("trip_id").unique(),
                on="trip_id",
                how="semi"
            )
            .collect()
        )

    def build_calendar(
        self,
        trips: pl.DataFrame
    ) -> pl.DataFrame:
        return (
            GtfsLoader.calendar()
            .join(
                trips.lazy().select("service_id").unique(),
                on="service_id",
                how="semi"
            )
            .collect()
        )

    def build_calendar_dates(
        self,
        trips: pl.DataFrame
    ) -> pl.DataFrame:
        return (
            GtfsLoader.calendar_dates()
            .join(
                trips.lazy().select("service_id").unique(),
                on="service_id",
                how="semi"
            )
            .collect()
        )

    def build_agencies(
        self,
        routes: pl.DataFrame
    ) -> pl.DataFrame:
        return (
            GtfsLoader.agencies()
            .join(
                routes.lazy().select("agency_id").unique(),
                on="agency_id",
                how="semi"
            )
            .collect()
        )

    def build_frequencies(
        self,
        trips: pl.DataFrame
    ) -> pl.DataFrame:
        return (
            GtfsLoader.frequencies()
            .join(
                trips.lazy().select("trip_id").unique(),
                on="trip_id",
                how="semi"
            )
            .collect()
        )