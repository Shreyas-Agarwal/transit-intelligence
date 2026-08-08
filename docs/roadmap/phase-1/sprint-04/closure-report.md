# Sprint 04 Closure Report

## Sprint Information

**Sprint:** Sprint 04
**Planned Period:** June 22, 2026 – June 28, 2026 (7 days)
**Actual Period:** June 21, 2026 – August 8, 2026 (49 days elapsed, closed retroactively)
**Planned Theme:** Transit Network Modeling Foundation
**Planned Branch:** `feat/network-modeling`
**Actual Branch(es):** `feat/static-ingestion`, `feat/realtime-ingestion`, `chore/adopt-domain-first-workspace`, `feat/repo-governance-tooling`, `feat/docs`, `feat/ingestion`

---

## Sprint Goal (as specified)

Transform the Zurich GTFS subset from a collection of transit tables into a formally modeled transit network, via:

```text
GTFS Static
    ↓
Semantic Modeling
    ↓
Network Construction
    ↓
Graph Artifacts
    ↓
Network Analytics
```

Five objectives were specified: analytical data profiling, a semantic model v1, a network representation ADR, a network construction pipeline, and baseline network analytics.

**None of the five were started.** What happened instead is documented below, and it's the reason this closure report exists in the first place — not to mark the sprint a failure, but to record plainly that the roadmap and the work diverged, and why that divergence was the right call.

---

## Summary

Sprint 04, as specified, did not happen. What is being closed here instead is 49 days of work that moved Transit Intelligence from a research prototype toward something that can operate as a real platform — and the honest acknowledgment that this happened without updating the plan to say so.

The sprint spec was committed on June 22, 2026. Two small, unrelated commits followed over the next eight days. Then the branch went quiet for 39 days. When work resumed, on August 8, 2026, 46 commits landed in a single day across five merged PRs: a Rust GTFS-static downloader, a Rust GTFS-realtime pipeline on Redpanda, a domain-first workspace reorganization (ADR 0013), a full repository governance and secret-scanning stack, and the retirement of the original TypeScript prototype. This branch (`feat/ingestion`) continued that same line of work — further ingestion domain splitting, a Bronze → Silver transform pipeline, and deletion of the now-superseded `gtfs_s` domain.

None of it is network modeling. All of it is infrastructure the network-modeling work — and everything after it — will now depend on. This is being framed as an unplanned but justified pivot: the project outgrew "prototype" mode and needed ingestion, architecture, and governance foundations before graph analytics could be built on anything durable. The gap in this report is that the pivot wasn't written down anywhere until now.

---

## Objectives Review

### Objective 1 – Analytical Data Profiling

**Status:** Not started; carried to Sprint 05.

**Expected deliverable:** `docs/research/gtfs-analytical-profiling.md`.

**Actual:** File does not exist yet. No agency-naming, route-classification, service-categorization, or stop-normalization analysis was performed this window. This is now first on Sprint 05's list, working against the Silver-tier Parquet output the new ingestion pipeline produces.

---

### Objective 2 – Semantic Model v1

**Status:** Not started; carried to Sprint 05.

**Expected deliverable:** `docs/design/transit-semantic-model.md`, defining entities, keys, and relationships for Agencies → Routes → Trips → Stop Times → Stops.

**Actual:** File does not exist yet. The transit domain model remains implicit in code. No regression here — just not this sprint's work.

---

### Objective 3 – Network Representation Design

**Status:** Not started; carried to Sprint 05 with a housekeeping correction.

**Expected deliverable:** ADR 0013, "Transit Network Representation," selecting a canonical graph model (stop graph vs. station graph vs. route graph).

**Actual:** ADR 0013 was written this window, but for a different, necessary decision: **"Adopt Domain-First Workspace Organization,"** the architectural call underpinning the ingestion rebuild. That decision needed an ADR and 0013 was next in sequence — a reasonable outcome of the pivot, with one loose end: the number the sprint-04 spec had reserved is now spoken for. Sprint 05 renumbers the network representation ADR to **0014**.

---

### Objective 4 – Network Construction Pipeline

**Status:** Not started; carried to Sprint 05, on firmer ground than before.

**Expected deliverable:** A pipeline producing `nodes.parquet` and `edges.parquet` from the GTFS static subset.

**Actual:** Neither artifact exists yet. What does exist now, and didn't at sprint start, is a real Bronze → Silver transform pipeline with validation gates, producing the versioned Zurich subset this objective will build on — a better foundation for a graph pipeline than the standalone `gtfs_s` scripts this sprint was originally scoped against.

---

### Objective 5 – Baseline Network Analytics

**Status:** Not started; blocked on Objective 4, as originally specified.

**Expected deliverable:** Degree, connected-components, and shortest-path-validation metrics over the constructed graph.

**Actual:** No graph exists yet to compute metrics over. Unchanged from the original dependency chain — this was always going to wait on Objective 4.

---

## What Actually Shipped Instead

For the record, since it consumed the entire sprint window and matters more than any one sprint's scorecard:

* **Static ingestion (Rust):** CKAN client, download/archive/manifest/lock/symlink handling, Parquet conversion — a from-scratch GTFS-static downloader domain.
* **Realtime ingestion (Rust):** GTFS-RT protobuf decoding, feed fetching, Redpanda producer/topics, CLI entrypoint — replacing the previous TypeScript realtime worker.
* **Domain-first workspace reorganization (ADR 0013):** supersedes ADR 0010, restructures the repository around bounded-context domains (`ingestion/extract`, `ingestion/transform`, etc.) instead of shared root-level language manifests.
* **Repository governance tooling:** Conventional Commits + DCO enforcement, markdownlint, editorconfig-checker, Vale prose linting, gitleaks secret scanning, Renovate, per-domain Makefiles as a CI convention, runtime domain discovery in CI, issue/PR templates, SECURITY.md.
* **Sprint 03 scope-creep resolution:** `apps/network-explorer` (Analytics Studio) and its supporting TypeScript `platform/` libraries have been removed from this repository entirely and now live as their own separate project, since they never had anything to do with Transit Intelligence's actual roadmap.
* **This branch (`feat/ingestion`):** further split of `extract`/`transform` within the ingestion domain, Bronze → Silver transform pipeline with Tier-2 validation, and full deletion of the legacy `gtfs_s` domain now that its logic has been ported.

This is the point where Transit Intelligence stops being a research prototype and starts being built like a platform: durable, versioned ingestion instead of ad hoc scripts; a domain-scoped architecture that can grow without dependency collisions; governance tooling that catches problems before they ship; and a clean split-off of the interesting-but-unrelated Analytics Studio prototype into its own project rather than letting it linger as permanent scope creep.

---

## Sprint Outcome

Sprint 04, as specified, delivered zero of five objectives. All five are carried forward to Sprint 05 (`docs/roadmap/phase-1/sprint-05/sprint-spec.md`), which they should be — the plan itself was sound, it just wasn't what this window turned out to need.

What actually happened this window was a necessary and overdue maturity step for the project, executed without updating the roadmap to reflect it. That's the one real gap here: not that the priority shifted, but that nothing was written down when it did. The fix isn't "don't deviate" — the ingestion rebuild was the right call. The fix is to make deviations like this visible in the plan the moment they happen, so a reader doesn't need `git log` to find out what a sprint actually became.
