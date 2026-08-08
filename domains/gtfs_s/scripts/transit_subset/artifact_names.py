from typing import Final


class ArtifactNames:
    ZURICH_STOPS: Final[str] = "stops/zurich_stops.parquet"
    ZURICH_TRIP_IDS: Final[str] = "trips/zurich_trip_ids.parquet"
    ZURICH_TRIPS: Final[str] = "trips/zurich_trips.parquet"
    INTERNAL_TRIPS: Final[str] = "trips/internal_trips.parquet"
    CROSSING_TRIPS: Final[str] = "trips/crossing_trips.parquet"
    ZURICH_ROUTES: Final[str] = "routes/zurich_routes.parquet"
    INTERNAL_ROUTES: Final[str] = "routes/internal_routes.parquet"
    CROSSING_ROUTES: Final[str] = "routes/crossing_routes.parquet"
    MIXED_ROUTES: Final[str] = "routes/mixed_routes.parquet"
    
    STOP_TIMES: Final[str] = "stop_times/zurich_stop_times.parquet"
    INTERNAL_STOP_TIMES: Final[str] = "stop_times/internal_stop_times.parquet"
    CROSSING_STOP_TIMES: Final[str] = "stop_times/crossing_stop_times.parquet"
    
    CALENDAR: Final[str] = "calendar/zurich_calendar.parquet"
    CALENDAR_DATES: Final[str] = "calendar_dates/zurich_calendar_dates.parquet"
    AGENCIES: Final[str] = "agencies/zurich_agencies.parquet"
    FREQUENCIES: Final[str] = "frequencies/zurich_frequencies.parquet"
