from transit_subset.artifact_names import ArtifactNames
from transit_subset.artifact_writer import ArtifactWriter
from transit_subset.logger import get_logger
from transit_subset.paths import GTFS_DIR
from transit_subset.subset_builder import ZurichSubsetBuilder

logger = get_logger(__name__)

def main() -> None:
    builder = ZurichSubsetBuilder()
    writer = ArtifactWriter()

    logger.info("Loading stops")
    logger.info("Building Zurich stop subset")
    stops = builder.build_stops()
    writer.write(stops, ArtifactNames.ZURICH_STOPS)

    logger.info("Building trip universe")
    trip_ids = builder.build_trip_ids(stops)
    writer.write(trip_ids, ArtifactNames.ZURICH_TRIP_IDS)

    trips = builder.build_trips(trip_ids)
    writer.write(trips, ArtifactNames.ZURICH_TRIPS)

    routes = builder.build_routes(trips)
    writer.write(routes, ArtifactNames.ZURICH_ROUTES)

    logger.info("Building classified trips (internal/crossing)")
    internal_trips, crossing_trips = builder.build_classified_trips(stops, trips)
    writer.write(internal_trips, ArtifactNames.INTERNAL_TRIPS)
    writer.write(crossing_trips, ArtifactNames.CROSSING_TRIPS)

    logger.info("Building classified routes (internal/crossing/mixed)")
    internal_routes, crossing_routes, mixed_routes = builder.build_classified_routes(
        routes, internal_trips, crossing_trips
    )
    writer.write(internal_routes, ArtifactNames.INTERNAL_ROUTES)
    writer.write(crossing_routes, ArtifactNames.CROSSING_ROUTES)
    writer.write(mixed_routes, ArtifactNames.MIXED_ROUTES)

    logger.info("Building Zurich stop_times subset")
    stop_times = builder.build_stop_times(trip_ids)
    writer.write(stop_times, ArtifactNames.STOP_TIMES)

    logger.info("Building internal stop_times subset")
    internal_stop_times = builder.build_stop_times(internal_trips)
    writer.write(internal_stop_times, ArtifactNames.INTERNAL_STOP_TIMES)

    logger.info("Building crossing stop_times subset")
    crossing_stop_times = builder.build_stop_times(crossing_trips)
    writer.write(crossing_stop_times, ArtifactNames.CROSSING_STOP_TIMES)

    logger.info("Building Zurich calendar subset")
    calendar = builder.build_calendar(trips)
    writer.write(calendar, ArtifactNames.CALENDAR)

    logger.info("Building Zurich calendar_dates subset")
    calendar_dates = builder.build_calendar_dates(trips)
    writer.write(calendar_dates, ArtifactNames.CALENDAR_DATES)

    logger.info("Building Zurich agencies subset")
    agencies = builder.build_agencies(routes)
    writer.write(agencies, ArtifactNames.AGENCIES)

    logger.info("Building Zurich frequencies subset")
    frequencies = builder.build_frequencies(trips)
    writer.write(frequencies, ArtifactNames.FREQUENCIES)

    writer.write_manifest()
    
    summary_stats = {
        "stops": stops.height,
        "trips": trips.height,
        "routes": routes.height,
        "stop_times": stop_times.height,
        "calendar": calendar.height,
        "calendar_dates": calendar_dates.height,
        "agencies": agencies.height,
        "frequencies": frequencies.height
    }
    writer.write_run_summary(summary_stats)

    print("\n" + "=" * 50)
    print("GTFS SUBSET PIPELINE SUMMARY")
    print("=" * 50 + "\n")
    print(f"Feed: {GTFS_DIR.name}\n")
    print("Stops:")
    print(f"  {stops.height:,}\n")
    print("Trips:")
    print(f"  {trips.height:,}\n")
    print(f"  - Internal: {internal_trips.height:,}")
    print(f"  - Crossing: {crossing_trips.height:,}\n")
    print("Routes:")
    print(f"  {routes.height:,}\n")
    print(f"  - Internal: {internal_routes.height:,}")
    print(f"  - Crossing: {crossing_routes.height:,}")
    print(f"  - Mixed: {mixed_routes.height:,}\n")
    
    print("Stop Times:")
    print(f"  {stop_times.height:,}")
    print(f"  - Internal: {internal_stop_times.height:,}")
    print(f"  - Crossing: {crossing_stop_times.height:,}\n")
    
    print("Calendar:")
    print(f"  {calendar.height:,}\n")
    
    print("Calendar Dates:")
    print(f"  {calendar_dates.height:,}\n")
    
    print("Agencies:")
    print(f"  {agencies.height:,}\n")
    
    print("Frequencies:")
    print(f"  {frequencies.height:,}\n")
    
    print("Artifacts Written:")
    print(f"  {writer.written_count}\n")
    print("Manifest:")
    print("  data/processed/metadata/manifest.json")


if __name__ == "__main__":
    main()
