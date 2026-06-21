import pytest
import polars as pl

@pytest.fixture
def mock_stops() -> pl.DataFrame:
    return pl.DataFrame({
        "stop_id": ["s1", "s2", "s3"],
        "stop_name": ["Zürich HB", "Zürich Oerlikon", "Bern HB"]
    })

@pytest.fixture
def mock_stop_times() -> pl.LazyFrame:
    return pl.LazyFrame({
        "trip_id": ["t1", "t1", "t2", "t3"],
        "stop_id": ["s1", "s2", "s3", "s1"]
    })

@pytest.fixture
def mock_trips() -> pl.LazyFrame:
    return pl.LazyFrame({
        "trip_id": ["t1", "t2", "t3"],
        "route_id": ["r1", "r2", "r1"]
    })

@pytest.fixture
def mock_routes() -> pl.LazyFrame:
    return pl.LazyFrame({
        "route_id": ["r1", "r2", "r3"]
    })
