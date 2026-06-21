import json
from datetime import datetime
from pathlib import Path
from typing import Any, Dict

import polars as pl

from transit_subset.paths import PROCESSED_DIR, GTFS_DIR
from transit_subset.logger import get_logger

logger = get_logger(__name__)

class ArtifactWriter:
    def __init__(self) -> None:
        self.metadata: Dict[str, Dict[str, Any]] = {}
        self.written_count: int = 0

    def write(self, df: pl.DataFrame, artifact_name: str) -> Dict[str, Any]:
        out_path = PROCESSED_DIR / artifact_name
        out_path.parent.mkdir(parents=True, exist_ok=True)
        
        df.write_parquet(out_path)
        
        file_size_bytes = out_path.stat().st_size
        
        rows = df.height
        columns = df.width
        name_key = Path(artifact_name).stem
        created_at = datetime.utcnow().isoformat() + "Z"
        
        meta = {
            "name": name_key,
            "path": artifact_name,
            "rows": rows,
            "columns": columns,
            "created_at": created_at,
            "file_size_bytes": file_size_bytes
        }
        
        self.metadata[name_key] = {
            "rows": rows,
            "columns": columns,
            "path": artifact_name,
            "generation_timestamp": created_at,
            "file_size_bytes": file_size_bytes
        }
        
        self.written_count += 1
        
        logger.info(
            f"Wrote artifact:\n       {artifact_name}\n       rows={rows:,}"
        )
        
        return meta

    def write_manifest(self) -> None:
        manifest_path = PROCESSED_DIR / "metadata" / "manifest.json"
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        
        data = {
            "generated_at": datetime.utcnow().isoformat() + "Z",
            "gtfs_feed": GTFS_DIR.name,
            "artifacts": self.metadata
        }
        
        with open(manifest_path, "w") as f:
            json.dump(data, f, indent=2)

    def write_run_summary(
        self,
        summary_stats: Dict[str, Any]
    ) -> None:
        summary_path = PROCESSED_DIR / "metadata" / "run_summary.json"
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        
        data = {
            "feed_name": GTFS_DIR.name,
            "generated_at": datetime.utcnow().isoformat() + "Z",
            **summary_stats
        }
        
        with open(summary_path, "w") as f:
            json.dump(data, f, indent=2)