"""Zurich operational subset derivation (ADR 0011) — ported from
`domains/gtfs_s/scripts/transit_subset/subset_builder.py`, which stays in
place unchanged. Same trip/route universe derivation, same
internal/crossing classification; the only difference is where the tables
come from: already-loaded Bronze Parquet DataFrames instead of `pl.scan_csv`
against a fixed `GTFS_DIR`.

Unlike trips/routes, `stops` is a fact table — every bronze row is kept.
Rather than filtering it down to "the Zurich stops", each stop is classified
via a `stop_type` column:

* `internal`  — matches the region's own membership test (today: stop_name
  starts with "Zürich"; `ZurichConfig.stop_prefix` is what to swap out for a
  different region).
* `boundary`  — outside the region, but touched by a trip that also touches
  an internal stop. This is where the region's network meets the rest of the
  feed (today: Zurich trips that continue on to Winterthur or Baden).
* `external`  — everything else: no relationship to the region's network at
  all.

This relationship (internal / touches-a-touching-trip / unrelated) is scope-
agnostic — expanding from Zurich to all of Switzerland only changes what
`internal` means (e.g. a canton list or a country-boundary test instead of a
name prefix); `boundary` and `external` fall out of the same trip-adjacency
logic unchanged, now classifying e.g. neighbouring-country stops instead of
neighbouring-canton ones.

Kept lazy through the joins (`.lazy()` / `.collect()` per step) per ADR 0011's
own processing guidance — predicate/projection pushdown and semi joins matter
here since `stop_times` is tens of millions of rows nationwide.
"""

from __future__ import annotations

from dataclasses import dataclass

import polars as pl


@dataclass(frozen=True)
class ZurichConfig:
    stop_prefix: str = "Zürich"


class ZurichSubsetBuilder:
    def __init__(self, tables: dict[str, pl.DataFrame], config: ZurichConfig | None = None) -> None:
        self.tables = tables
        self.config = config or ZurichConfig()

    def build_internal_stop_ids(self) -> pl.DataFrame:
        """`stop_id`s of stops that are `internal` to the region — the
        region's own membership test, today a `stop_name` prefix match."""
        return (
            self.tables["stops"]
            .filter(pl.col("stop_name").str.starts_with(self.config.stop_prefix))
            .select("stop_id")
        )

    def build_boundary_stop_ids(
        self, internal_stop_ids: pl.DataFrame, touching_trip_ids: pl.DataFrame
    ) -> pl.DataFrame:
        """`stop_id`s that are not internal but are visited by a trip that
        also visits an internal stop — i.e. where the region's network meets
        the rest of the feed."""
        return (
            self.tables["stop_times"]
            .lazy()
            .join(touching_trip_ids.lazy(), on="trip_id", how="semi")
            .join(internal_stop_ids.lazy(), on="stop_id", how="anti")
            .select("stop_id")
            .unique()
            .collect()
        )

    def build_stops(
        self, internal_stop_ids: pl.DataFrame, boundary_stop_ids: pl.DataFrame
    ) -> pl.DataFrame:
        """Every bronze stop, tagged with `stop_type` — `internal`,
        `boundary`, or `external`. No rows are dropped."""
        stop_type_lookup = pl.concat(
            [
                internal_stop_ids.select("stop_id").with_columns(pl.lit("internal").alias("stop_type")),
                boundary_stop_ids.select("stop_id").with_columns(pl.lit("boundary").alias("stop_type")),
            ]
        )
        return (
            self.tables["stops"]
            .lazy()
            .join(stop_type_lookup.lazy(), on="stop_id", how="left")
            .with_columns(pl.col("stop_type").fill_null("external"))
            .collect()
        )

    def build_internal_stops(self, stops: pl.DataFrame) -> pl.DataFrame:
        """The `internal`-only slice of `stops` — the operational-scope
        subset (ADR 0011's original "Zurich stops") that `trip_ids`,
        `internal_trips`, etc. are actually derived from. Kept as its own
        artifact so consumers that want "just the region's stops" don't have
        to re-filter the fact table themselves."""
        return stops.filter(pl.col("stop_type") == "internal").drop("stop_type")

    def build_trip_ids(self, stop_ids: pl.DataFrame) -> pl.DataFrame:
        return (
            self.tables["stop_times"]
            .lazy()
            .join(stop_ids.lazy().select("stop_id"), on="stop_id", how="semi")
            .select("trip_id")
            .unique()
            .collect()
        )

    def build_trips(self, trip_ids: pl.DataFrame) -> pl.DataFrame:
        return (
            self.tables["trips"]
            .lazy()
            .join(trip_ids.lazy(), on="trip_id", how="semi")
            .collect()
        )

    def build_routes(self, trips: pl.DataFrame) -> pl.DataFrame:
        return (
            self.tables["routes"]
            .lazy()
            .join(trips.lazy().select("route_id").unique(), on="route_id", how="semi")
            .collect()
        )

    def build_classified_trips(
        self, internal_stop_ids: pl.DataFrame, zurich_trips: pl.DataFrame
    ) -> tuple[pl.DataFrame, pl.DataFrame]:
        stop_times = self.tables["stop_times"].lazy()

        relevant_stop_times = stop_times.join(
            zurich_trips.lazy().select("trip_id"), on="trip_id", how="semi"
        )

        trip_stop_counts = relevant_stop_times.group_by("trip_id").agg(
            pl.len().alias("total_stops")
        )

        zurich_stop_counts = (
            relevant_stop_times.join(
                internal_stop_ids.lazy().select("stop_id"), on="stop_id", how="semi"
            )
            .group_by("trip_id")
            .agg(pl.len().alias("zurich_stops"))
        )

        trip_classification = trip_stop_counts.join(
            zurich_stop_counts, on="trip_id", how="inner"
        ).with_columns(
            pl.when(pl.col("total_stops") == pl.col("zurich_stops"))
            .then(pl.lit("internal"))
            .otherwise(pl.lit("crossing"))
            .alias("trip_type")
        )

        classified_trips = (
            zurich_trips.lazy().join(trip_classification, on="trip_id", how="inner").collect()
        )

        internal_trips = classified_trips.filter(pl.col("trip_type") == "internal").drop(
            "trip_type", "total_stops", "zurich_stops"
        )
        crossing_trips = classified_trips.filter(pl.col("trip_type") == "crossing").drop(
            "trip_type", "total_stops", "zurich_stops"
        )

        return internal_trips, crossing_trips

    def build_classified_routes(
        self,
        zurich_routes: pl.DataFrame,
        internal_trips: pl.DataFrame,
        crossing_trips: pl.DataFrame,
    ) -> tuple[pl.DataFrame, pl.DataFrame, pl.DataFrame]:
        internal = internal_trips.lazy().with_columns(pl.lit("internal").alias("trip_type"))
        crossing = crossing_trips.lazy().with_columns(pl.lit("crossing").alias("trip_type"))

        classified_trips = pl.concat([internal, crossing])

        route_classification = (
            classified_trips.group_by("route_id")
            .agg(pl.col("trip_type").unique())
            .with_columns(
                pl.when(pl.col("trip_type").list.len() == 1)
                .then(pl.col("trip_type").list.first())
                .otherwise(pl.lit("mixed"))
                .alias("classification")
            )
        )

        classified_routes = (
            zurich_routes.lazy().join(route_classification, on="route_id", how="inner").collect()
        )

        internal_routes = classified_routes.filter(pl.col("classification") == "internal").drop(
            "classification", "trip_type"
        )
        crossing_routes = classified_routes.filter(pl.col("classification") == "crossing").drop(
            "classification", "trip_type"
        )
        mixed_routes = classified_routes.filter(pl.col("classification") == "mixed").drop(
            "classification", "trip_type"
        )

        return internal_routes, crossing_routes, mixed_routes

    def build_stop_times(self, trip_ids: pl.DataFrame) -> pl.DataFrame:
        return (
            self.tables["stop_times"]
            .lazy()
            .join(trip_ids.lazy().select("trip_id").unique(), on="trip_id", how="semi")
            .collect()
        )

    def build_calendar(self, trips: pl.DataFrame) -> pl.DataFrame | None:
        if "calendar" not in self.tables:
            return None
        return (
            self.tables["calendar"]
            .lazy()
            .join(trips.lazy().select("service_id").unique(), on="service_id", how="semi")
            .collect()
        )

    def build_calendar_dates(self, trips: pl.DataFrame) -> pl.DataFrame | None:
        if "calendar_dates" not in self.tables:
            return None
        return (
            self.tables["calendar_dates"]
            .lazy()
            .join(trips.lazy().select("service_id").unique(), on="service_id", how="semi")
            .collect()
        )

    def build_agencies(self, routes: pl.DataFrame) -> pl.DataFrame | None:
        if "agency" not in self.tables:
            return None
        return (
            self.tables["agency"]
            .lazy()
            .join(routes.lazy().select("agency_id").unique(), on="agency_id", how="semi")
            .collect()
        )

    def build_frequencies(self, trips: pl.DataFrame) -> pl.DataFrame | None:
        if "frequencies" not in self.tables:
            return None
        return (
            self.tables["frequencies"]
            .lazy()
            .join(trips.lazy().select("trip_id").unique(), on="trip_id", how="semi")
            .collect()
        )


@dataclass
class ZurichSubset:
    """Every artifact ADR 0011 defines, named flat (matching the Bronze
    snapshot's own flat `stops.parquet` / `trips.parquet` / ... convention) —
    `None` for an optional artifact whose source table wasn't present in
    this snapshot.

    `stops` is the one exception to "subset": it's every bronze stop, tagged
    with `stop_type` (`internal`/`boundary`/`external`), not filtered down.
    `internal_stops` is the actual operational-scope subset — what `trips`,
    `routes`, `stop_times`, etc. are all derived from — kept as its own
    artifact rather than folded into `stops`.
    """

    stops: pl.DataFrame
    internal_stops: pl.DataFrame
    trip_ids: pl.DataFrame
    trips: pl.DataFrame
    internal_trips: pl.DataFrame
    crossing_trips: pl.DataFrame
    routes: pl.DataFrame
    internal_routes: pl.DataFrame
    crossing_routes: pl.DataFrame
    mixed_routes: pl.DataFrame
    stop_times: pl.DataFrame
    internal_stop_times: pl.DataFrame
    crossing_stop_times: pl.DataFrame
    calendar: pl.DataFrame | None
    calendar_dates: pl.DataFrame | None
    agencies: pl.DataFrame | None
    frequencies: pl.DataFrame | None

    def artifacts(self) -> dict[str, pl.DataFrame]:
        """Name -> DataFrame for every artifact actually produced (skips the
        `None` optional ones)."""
        return {
            name: df
            for name, df in vars(self).items()
            if df is not None
        }


def build_zurich_subset(
    tables: dict[str, pl.DataFrame], config: ZurichConfig | None = None
) -> ZurichSubset:
    builder = ZurichSubsetBuilder(tables, config)

    internal_stop_ids = builder.build_internal_stop_ids()
    trip_ids = builder.build_trip_ids(internal_stop_ids)
    trips = builder.build_trips(trip_ids)
    routes = builder.build_routes(trips)

    internal_trips, crossing_trips = builder.build_classified_trips(internal_stop_ids, trips)
    internal_routes, crossing_routes, mixed_routes = builder.build_classified_routes(
        routes, internal_trips, crossing_trips
    )

    boundary_stop_ids = builder.build_boundary_stop_ids(internal_stop_ids, trip_ids)
    stops = builder.build_stops(internal_stop_ids, boundary_stop_ids)
    internal_stops = builder.build_internal_stops(stops)

    stop_times = builder.build_stop_times(trip_ids)
    internal_stop_times = builder.build_stop_times(internal_trips)
    crossing_stop_times = builder.build_stop_times(crossing_trips)

    return ZurichSubset(
        stops=stops,
        internal_stops=internal_stops,
        trip_ids=trip_ids,
        trips=trips,
        internal_trips=internal_trips,
        crossing_trips=crossing_trips,
        routes=routes,
        internal_routes=internal_routes,
        crossing_routes=crossing_routes,
        mixed_routes=mixed_routes,
        stop_times=stop_times,
        internal_stop_times=internal_stop_times,
        crossing_stop_times=crossing_stop_times,
        calendar=builder.build_calendar(trips),
        calendar_dates=builder.build_calendar_dates(trips),
        agencies=builder.build_agencies(routes),
        frequencies=builder.build_frequencies(trips),
    )
