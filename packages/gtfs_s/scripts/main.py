from transit_subset.subset_builder import ZurichSubsetBuilder
from transit_subset.artifact_writer import ArtifactWriter
from transit_subset.artifact_names import ArtifactNames
from transit_subset.logger import get_logger
from transit_subset.paths import GTFS_DIR

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

    writer.write_manifest()
    writer.write_run_summary(
        stops_count=stops.height,
        trips_count=trips.height,
        routes_count=routes.height
    )

    print("\n" + "=" * 50)
    print("GTFS SUBSET PIPELINE SUMMARY")
    print("=" * 50 + "\n")
    print(f"Feed: {GTFS_DIR.name}\n")
    print("Stops:")
    print(f"  {stops.height:,}\n")
    print("Trips:")
    print(f"  {trips.height:,}\n")
    print("Routes:")
    print(f"  {routes.height:,}\n")
    print("Artifacts Written:")
    print(f"  {writer.written_count}\n")
    print("Manifest:")
    print("  data/processed/metadata/manifest.json")


if __name__ == "__main__":
    main()
