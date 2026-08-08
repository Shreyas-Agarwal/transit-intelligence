from __future__ import annotations

import polars as pl

from ingestion.transform.validate import validate_snapshot


def test_good_tables_pass(good_tables):
    report = validate_snapshot("20260805", good_tables)
    assert report.passed, report.failures


def test_missing_required_column_fails(good_tables):
    tables = dict(good_tables)
    tables["stops"] = tables["stops"].drop("stop_lat")

    report = validate_snapshot("20260805", tables)

    assert not report.passed
    assert any(f.table == "stops" and f.check == "required_columns" for f in report.failures)


def test_null_in_required_column_fails(good_tables):
    tables = dict(good_tables)
    stops = tables["stops"].clone()
    stops[0, "stop_name"] = None
    tables["stops"] = stops

    report = validate_snapshot("20260805", tables)

    assert not report.passed
    assert any(f.table == "stops" and f.check == "not_null" for f in report.failures)


def test_row_count_out_of_range_fails(good_tables):
    tables = dict(good_tables)
    tables["routes"] = tables["routes"].head(2)  # far below the 500-row floor

    report = validate_snapshot("20260805", tables)

    assert not report.passed
    assert any(f.table == "routes" and f.check == "row_count_range" for f in report.failures)


def test_orphaned_route_id_fails_referential_integrity(good_tables):
    tables = dict(good_tables)
    trips = tables["trips"].clone()
    trips[0, "route_id"] = "R_DOES_NOT_EXIST"
    tables["trips"] = trips

    report = validate_snapshot("20260805", tables)

    assert not report.passed
    assert any(
        f.table == "trips" and f.check == "referential_integrity" for f in report.failures
    )


def test_missing_table_does_not_crash_referential_integrity():
    # No "routes" table at all — referential integrity check should skip, not raise.
    tables = {
        "trips": pl.DataFrame({"trip_id": ["T1"], "route_id": ["R1"]}),
    }
    report = validate_snapshot("20260805", tables)
    assert not any(f.check == "referential_integrity" for f in report.failures)
