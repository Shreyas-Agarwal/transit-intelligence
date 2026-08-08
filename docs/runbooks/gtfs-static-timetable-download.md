# Runbook: Downloading the Swiss GTFS Static Timetable

## Purpose

The GTFS Static preprocessing pipeline (`domains/gtfs_s/scripts`, see [ADR 0011](../adr/0011-gtfs-static-preprocessing-and-zurich-subset-strategy.md)) requires a local copy of the nationwide Swiss GTFS-S ("Fahrplan") feed under `domains/gtfs_s/raw/`.

This feed is **not static despite the name** — opentransportdata.swiss republishes a new snapshot roughly twice a week. This runbook describes where the data comes from, how it is currently fetched by hand, and the `latest` symlink convention the pipeline relies on.

For the plan to stop doing this by hand, see the design doc: [GTFS Static Auto-Downloader & Updater](../design/gtfs-static-auto-downloader.md).

---

## Source

Dataset page:

```text
https://data.opentransportdata.swiss/dataset/timetable-2026-gtfs2020
```

Key facts about this dataset (confirmed 2026-08-08):

* Publisher: opentransportdata.swiss (Swiss national open transport data platform), backed by a CKAN catalog.
* Update cadence: **twice per week** ("Zweimal pro Woche"), no updates on Swiss public holidays.
* Each publish is a full nationwide GTFS-S snapshot, not a diff.
* Resource (file) naming convention:

  ```text
  GTFS_FP2026_YYYYMMDD.zip
  ```

  e.g. `GTFS_FP2026_20260805.zip`. Older resources (pre ~2025-09) used a hyphenated date suffix (`GTFS_FP2026_2025-09-22.zip`) — treat both forms as valid when parsing dates.

* Older snapshots roll off the live dataset page and are moved to `archive.opentransportdata.swiss`.
* The dataset also exposes a CKAN `package_show` API (`/api/3/action/package_show?id=timetable-2026-gtfs2020`), which is the intended machine-readable way to enumerate resources instead of scraping the HTML page. Direct anonymous fetches to this endpoint have returned `403` from some clients (e.g. Claude's `WebFetch` tool) — this needs to be re-verified with a normal HTTP client/User-Agent before automation depends on it (see open questions in the design doc).

---

## Where it lives locally

```text
domains/gtfs_s/
  raw/          # gitignored — downloaded snapshots live here
    gtfs_fp2026_20260805/
      stops.txt
      trips.txt
      routes.txt
      stop_times.txt
      calendar_dates.txt
      ...
    gtfs_fp2026_20260729/
      ...
  processed/    # gitignored — pipeline output (Parquet artifacts)
```

`domains/gtfs_s/scripts/transit_subset/paths.py` currently discovers the timetable to process by globbing `raw/gtfs_fp*` and sorting descending:

```python
GTFS_DIR = sorted(
    RAW_DIR.glob("gtfs_fp*"),
    reverse=True
)[0]
```

This works today because directory names sort lexicographically the same as chronologically (`gtfs_fp2026_20260805` > `gtfs_fp2026_20260729`), but it means **every consumer of the raw feed has to re-derive "which version is current"** by re-globbing and re-sorting. That is the problem the `latest` symlink (below) and the future auto-updater are meant to solve.

---

## `latest` symlink convention

Going forward, `domains/gtfs_s/raw/` should contain one directory per downloaded snapshot, named after the extracted zip (lowercased, e.g. `gtfs_fp2026_20260805/`), plus a symlink:

```text
domains/gtfs_s/raw/latest -> gtfs_fp2026_20260805/
```

Consumers (the subset builder, notebooks, ad-hoc analysis) should read from `domains/gtfs_s/raw/latest/` instead of globbing. This gives us one indirection point to repoint whenever a new snapshot is downloaded, and makes "what version are we running against" a `readlink` away instead of a glob-and-sort.

> `paths.py` has not been switched over to the symlink yet — it still globs. Updating it is in scope for the auto-downloader work (see design doc), so both mechanisms should keep agreeing (i.e. `latest` should always point at the same directory the glob would resolve to) until the cutover happens.

---

## Manual download procedure (current, pre-automation)

Until the auto-downloader exists, fetch a new snapshot by hand when the pipeline needs fresher data:

1. Open the dataset page and find the most recent resource:

   ```text
   https://data.opentransportdata.swiss/dataset/timetable-2026-gtfs2020
   ```

2. Download the zip (e.g. `GTFS_FP2026_20260805.zip`).

3. Extract it into a new, lowercased, date-suffixed directory under `domains/gtfs_s/raw/`:

   ```bash
   mkdir -p domains/gtfs_s/raw/gtfs_fp2026_20260805
   unzip GTFS_FP2026_20260805.zip -d domains/gtfs_s/raw/gtfs_fp2026_20260805
   ```

4. Repoint the `latest` symlink at the new directory (atomically, so nothing reading through `latest` ever sees a half-updated target):

   ```bash
   cd domains/gtfs_s/raw
   ln -sfn gtfs_fp2026_20260805 latest
   ```

5. Re-run the subset pipeline (`domains/gtfs_s/scripts`, see its [README](../../domains/gtfs_s/scripts/README.md)) so `processed/` reflects the new snapshot.

6. Old snapshot directories can be left in place for now (disk permitting) — retention policy is an open question for the design doc, not this runbook.

---

## Related documents

* [ADR 0011 — GTFS Static Preprocessing and Zurich Operational Subset Strategy](../adr/0011-gtfs-static-preprocessing-and-zurich-subset-strategy.md) — lists "Static Feed Automation" as future work; this runbook and the design doc below are that follow-through.
* [Design: GTFS Static Auto-Downloader & Updater](../design/gtfs-static-auto-downloader.md) — the plan for replacing the manual steps above with a script.
