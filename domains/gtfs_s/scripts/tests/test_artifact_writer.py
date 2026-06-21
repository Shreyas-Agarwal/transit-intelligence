import json
from unittest.mock import patch, MagicMock
from pathlib import Path
import polars as pl
from transit_subset.artifact_writer import ArtifactWriter
from transit_subset.artifact_names import ArtifactNames

@patch("transit_subset.artifact_writer.PROCESSED_DIR")
def test_write(mock_processed_dir, tmp_path):
    mock_processed_dir.__truediv__.side_effect = lambda x: tmp_path / x
    
    writer = ArtifactWriter()
    df = pl.DataFrame({"a": [1, 2, 3]})
    
    meta = writer.write(df, ArtifactNames.ZURICH_STOPS)
    
    assert meta["rows"] == 3
    assert meta["columns"] == 1
    assert "created_at" in meta
    assert writer.written_count == 1
    
    out_path = tmp_path / ArtifactNames.ZURICH_STOPS
    assert out_path.exists()
    
    # Verify the contents
    written_df = pl.read_parquet(out_path)
    assert written_df.height == 3

@patch("transit_subset.artifact_writer.GTFS_DIR")
@patch("transit_subset.artifact_writer.PROCESSED_DIR")
def test_write_manifest(mock_processed_dir, mock_gtfs_dir, tmp_path):
    mock_processed_dir.__truediv__.side_effect = lambda x: tmp_path / x
    mock_gtfs_dir.name = "test_feed"
    
    writer = ArtifactWriter()
    writer.metadata = {"test_artifact": {"rows": 1, "columns": 2, "path": "test.parquet"}}
    
    writer.write_manifest()
    
    manifest_path = tmp_path / "metadata" / "manifest.json"
    assert manifest_path.exists()
    
    with open(manifest_path, "r") as f:
        data = json.load(f)
        
    assert data["gtfs_feed"] == "test_feed"
    assert "generated_at" in data
    assert "test_artifact" in data["artifacts"]

@patch("transit_subset.artifact_writer.GTFS_DIR")
@patch("transit_subset.artifact_writer.PROCESSED_DIR")
def test_write_run_summary(mock_processed_dir, mock_gtfs_dir, tmp_path):
    mock_processed_dir.__truediv__.side_effect = lambda x: tmp_path / x
    mock_gtfs_dir.name = "test_feed"
    
    writer = ArtifactWriter()
    
    writer.write_run_summary(stops_count=10, trips_count=20, routes_count=5)
    
    summary_path = tmp_path / "metadata" / "run_summary.json"
    assert summary_path.exists()
    
    with open(summary_path, "r") as f:
        data = json.load(f)
        
    assert data["feed_name"] == "test_feed"
    assert data["stops"] == 10
    assert data["trips"] == 20
    assert data["routes"] == 5
