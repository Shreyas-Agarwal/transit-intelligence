# C4 — Dynamic: One Check-and-Update Run

Mermaid has no native C4 "Dynamic" diagram type, so this is a sequence diagram — the standard stand-in for a C4
dynamic view. It traces a single invocation of `ckan::pipeline::run`, in the order the code actually executes,
including the recovery steps the design doc requires to run *before* any network call (§12).

```mermaid
sequenceDiagram
    actor Scheduler
    participant CLI as ckan (main)
    participant Pipeline
    participant Lock
    participant Manifest
    participant Symlink
    participant CkanClient
    participant Download
    participant Archive
    participant ParquetConvert
    participant Raw as Raw Snapshot Store

    Scheduler->>CLI: invoke
    CLI->>Pipeline: run(layout, ckan_client, download_http)

    Pipeline->>Lock: acquire (O_CREAT|O_EXCL)
    alt lock file already exists
        Lock->>Lock: read PID from existing lock
        alt PID not running on this host
            Lock->>Raw: remove stale lock
            Lock->>Raw: create lock (retry)
        else PID running / different host
            Lock-->>Pipeline: error — another run in progress
        end
    end

    Note over Pipeline,Raw: Recovery — always runs, before any network call (§12)
    Pipeline->>Raw: wipe .staging/ unconditionally
    Pipeline->>Manifest: scan_sidecars(raw/*/.snapshot-meta.json)
    Manifest-->>Pipeline: installed versions
    Pipeline->>Manifest: rebuild_manifest(installed)
    Pipeline->>Raw: write .manifest.json
    Pipeline->>Symlink: read_latest()
    Symlink-->>Pipeline: current target (or none)
    Pipeline->>Pipeline: assert manifest.latest == symlink target (fail loudly on mismatch)

    Pipeline->>CkanClient: list_gtfs_zip_resources()
    CkanClient->>CkanClient: GET package_show (Bearer token, retried on failure)
    CkanClient-->>Pipeline: [UpstreamResource]
    Pipeline->>Pipeline: filter out already-installed versions, sort oldest-first

    loop for each pending version
        Pipeline->>Download: download_to_staging(url)
        Download-->>Pipeline: bytes, sha256, etag, last_modified
        Pipeline->>Pipeline: verify_upstream_hash (if CKAN hash present)
        Pipeline->>Archive: validate_and_extract(zip, staging_dir)
        Archive->>Archive: CRC32 check every entry
        Archive->>Archive: check required GTFS members present & non-empty
        Archive-->>Pipeline: ok / error
        alt validation failed
            Pipeline->>Raw: delete staging artifacts, record failed (in-memory, this run only)
        else validation passed
            Pipeline->>ParquetConvert: convert_directory(csv_staging, parquet_staging)
            ParquetConvert->>ParquetConvert: for every *.txt: read as all-Utf8, write *.parquet (zstd)
            ParquetConvert-->>Pipeline: ok / error
            alt conversion failed
                Pipeline->>Raw: delete CSV + parquet staging artifacts, record failed (in-memory, this run only)
            else conversion succeeded
                Pipeline->>Raw: delete CSV staging (scratch, no longer needed)
                Pipeline->>Raw: atomic rename parquet staging → raw/<version>/
                Pipeline->>Raw: write .snapshot-meta.json sidecar
            end
        end
    end

    Pipeline->>Pipeline: newest = max(installed versions with status verified)
    alt newest differs from current latest
        Pipeline->>Symlink: advance_latest(newest)
        Symlink->>Raw: symlink + rename (atomic swap)
    end

    Pipeline->>Manifest: rebuild_manifest(installed, failed_this_run)
    Pipeline->>Raw: write .manifest.json
    Pipeline->>Lock: release (on drop)
    Pipeline-->>CLI: Ok / Err
```

## Notes

* The lock-staleness branch and the manifest-rebuild-and-verify block both run **every** invocation, not just
  after a crash — this is what makes the design self-healing rather than needing a special "recovery mode"
  (design doc §12: "on every run, before touching the network, clean staging, reconcile the manifest against the
  sidecars, and verify `latest` agrees with the manifest").
* The per-version loop is sequential in the current implementation (correctness/simplicity favored over
  throughput, matching the "performance is not a concern" priority) — a failure on one version doesn't abort the
  run; it's recorded and the loop continues to the next version.
* `latest` only ever advances forward, and only after the loop finishes examining every pending version — never
  mid-loop and never backwards (design doc §7).
