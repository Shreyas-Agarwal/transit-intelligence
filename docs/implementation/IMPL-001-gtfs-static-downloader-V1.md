# IMPL-001: GTFS Static Downloader V2 — Implementation Log

This document tracks the phase-by-phase implementation of the GTFS Static Downloader v2 plan. Each section is the review-checkpoint report produced at the end of that phase. The existing system is documented in [DD-001-gtfs-static-downloader.md](../design/DD-001-gtfs-static-downloader.md); this log records what changed on top of it and why.

---

## Phase 0 — Repository and current-state reconnaissance

**Status: Complete. No code changed.** This phase was read-only reconnaissance of the `ckan` crate at `domains/ingestion/extract/ckan/`, cross-checked against [DD-001-gtfs-static-downloader.md](../design/DD-001-gtfs-static-downloader.md) and [ADR 0011](../architecture/adr/0011-gtfs-static-preprocessing-and-zurich-subset-strategy.md).

### Current architecture

Single binary (`ckan::main`), one process per invocation, no persistent daemon. `pipeline::run()` is the entire lifecycle:

```
main() → CkanConfig::from_env() → pipeline::run()
  1. acquire UpdaterLock (.updater.lock)
  2. clean_staging()                         — wipe .staging/ unconditionally
  3. manifest::scan_sidecars()                — filesystem is ground truth for "installed"
  4. rebuild + write manifest, verify_latest_consistency()  — fail loudly on mismatch
  5. ckan_client.list_gtfs_zip_resources()    — Discover
  6. filter by cutoff, filter by already-installed → Reconcile
  7. for each pending version (bounded by Arc<Semaphore>):
       spawn Tokio task → process_version()   — Download→Verify→Extract→Validate→Convert→Publish
  8. join all tasks (JoinSet::join_next)
  9. advance_latest_if_needed()               — max verified version, never backwards
  10. rebuild + write final manifest
  11. lock released on Drop
```

Mapped onto the 9 conceptual stages used to plan this work:

| Stage | File | Notes |
|---|---|---|
| 1. Discover | `ckan_client.rs::list_gtfs_zip_resources` | CKAN `package_show`, retried 3× via `ti_common::retry`, parses filenames via `domain::parse_resource_filename` |
| 2. Reconcile | `pipeline.rs::run` (cutoff filter + `installed.contains_key`) | Pure in-memory filter each run; no persisted "pending" state between runs |
| 3. Download | `download.rs::download_to_staging` | Async streaming to `.zip.part`, atomic rename to `.zip` on completion |
| 4. Verify | `download.rs` (Content-Length check) + `domain.rs::verify_upstream_hash` (SHA-256) | Verify happens *before* extraction, one stage, split across two files |
| 5. Extract | `archive.rs::validate_and_extract` | Extraction and validation are one function, not separable stages currently |
| 6. Validate | `archive.rs` (`validate_members` pre-extract CRC pass + `verify_extracted_members` post-extract) | Archive-level only, no GTFS semantic checks (explicit non-goal) |
| 7. Convert | `parquet_convert.rs::convert_directory` | Whole-file-in-memory, Utf8-only columns, ZSTD compression |
| 8. Publish | `pipeline.rs::process_version` (rename) + `manifest::write_sidecar` | Atomic `rename()`, staging→final, sidecar written immediately after |
| 9. Manifest/latest | `manifest.rs` + `symlink.rs` + `pipeline.rs::advance_latest_if_needed` | Manifest rebuilt from sidecars every run; symlink swap via temp-symlink+rename |

### Current state machine

There isn't a persisted per-version state machine today. State is **derived, not stored**:

- A version is "installed" iff `raw/<version>/` exists **and** has a valid `.snapshot-meta.json` (`manifest.rs::scan_sidecars`). No sidecar → not installed, regardless of directory contents.
- There is no `DISCOVERED`/`QUEUED`/`RUNNING` durable record. A version is implicitly "pending" for exactly one run: computed each invocation as `eligible - installed`.
- Failure is not durable either: `failed_this_run` is an in-memory `BTreeMap` that only survives long enough to render the end-of-run manifest snapshot. A version that failed on a previous run has zero record of that fact on the next run — it's simply attempted again because it's still not installed.
- `SidecarStatus` enum has `Verified` and `Failed`, but `Failed` is documented as never actually serialized (`manifest.rs:26-29`) — a failed version never gets a final directory or sidecar.
- `ManifestStatus` adds `Superseded`, computed at manifest-rebuild time (not persisted per-version) by comparing to `latest`.

So today's "state machine" is really: **filesystem presence-or-absence, recomputed from scratch every run.**

### Current concurrency model

- Single `Arc<Semaphore>` bounding total in-flight versions (`GTFS_S_MAX_CONCURRENT_VERSIONS`, default `min(4, available_parallelism)`).
- One Tokio task per version, spawned into a `JoinSet`; the permit is held for the entire per-version pipeline (download+extract+convert), released only when the task finishes.
- `tokio::task::spawn_blocking` wraps archive extraction and Parquet conversion (CPU/disk-bound sync work) so they don't block the async executor.
- No resource-specific pools: download concurrency and extract/convert concurrency share the same semaphore/slot. A version that's converting occupies the same "slot type" as a version that's downloading.
- All shared mutable state (manifest, `latest`, `installed` map) is touched only *after* `JoinSet::join_next()` drains — i.e., serialized in the main task, never from worker tasks.
- Producer isn't decoupled from execution: `pipeline::run` spawns a task per pending version directly (bounded by permit acquisition blocking the spawn loop) — there's no separate queue data structure, just semaphore-gated spawning.

### Current locking model

- Single global `.updater.lock` file (`lock.rs`), exclusive-create semantics, holding PID + hostname + timestamp.
- One retry on contention: if the existing lock's PID isn't alive on the same host, it's deleted and acquisition retried once.
- Cross-host contention (different hostname) is always treated as held — no way to verify liveness remotely.
- Released via `Drop` (RAII), covering both success and panic-unwind paths.
- This lock only serializes whole **invocations**, not individual versions — it does not implement per-version ownership/leasing.

### Current recovery model

- Fully stateless/idempotent-by-recomputation: `clean_staging()` deletes `.staging/` unconditionally at the start of every run, regardless of whether the prior run crashed or exited cleanly. Nothing is resumed — a partially downloaded/extracted/converted version is discarded and reprocessed wholesale.
- Stale lock recovery: PID-liveness check via `/proc/<pid>` (Linux-only, acceptable per code comment).
- Missing/corrupt manifest: fully rebuilt from sidecars every run (never trusted as authoritative even when present).
- `latest`/manifest mismatch: **fails loudly** (`PipelineError::LatestMismatch`) rather than guessing — this is a deliberate invariant, not a bug.
- A snapshot directory without a valid sidecar is never treated as installed, so it will be silently overwritten by a freshly-validated copy on next successful processing (`pipeline.rs:420-434`, logged as a warning).
- There is no crash-recovery granularity finer than "redo the whole version." No stage-boundary state is persisted (Phase 6 in the plan targets exactly this gap).

### Existing test coverage

All 34 non-ignored tests pass; 3 wall-clock benchmarks are `#[ignore]`d by design.

| File | Count | Covers |
|---|---|---|
| `src/domain.rs` (unit) | 4 | filename parsing (current + legacy hyphenated), version ordering |
| `src/config.rs` (unit) | 6 | env parsing, cutoff parsing, concurrency default/override, raw_dir resolution |
| `src/pipeline.rs` (unit) | 2 | `format_bytes`/`format_duration` helpers only — no unit tests of `run`/`process_version` logic itself |
| `tests/archive.rs` | 4 | Tier-1 validation: valid zip, missing member, empty member, non-zip |
| `tests/parquet_convert.rs` | 2 | CSV→Parquet conversion, string round-trip, row-count preservation |
| `tests/symlink.rs` | 2 | atomic advance, no leftover temp symlink |
| `tests/lock.rs` | 3 | acquire/release round trip, stale-lock self-heal, live-PID contention |
| `tests/manifest_recovery.rs` | 3 | manifest rebuild after deletion, latest = highest version not upload order, dir-without-sidecar not installed |
| `tests/pipeline_concurrent.rs` | 8 | full pipeline via a **synchronous re-implementation** (`run_version_pipeline_sync`) of `process_version`'s steps — not calling `pipeline::process_version` or `pipeline::run` directly; covers success, failure isolation, latest-by-version-id-not-completion-order, staging cleanup on failure, startup staging wipe |
| `tests/benchmark_concurrent.rs` | 3 (ignored) | sequential vs. concurrent wall-clock comparison |

**Coverage gap worth flagging for later phases**: nothing exercises `pipeline::run` or `pipeline::process_version` end-to-end through Tokio (no mock HTTP layer for CKAN or download); `pipeline_concurrent.rs` duplicates the pipeline logic synchronously rather than calling the real async functions, so a bug introduced only in `process_version`'s orchestration (not in the underlying `archive`/`parquet_convert`/`manifest` calls it makes) wouldn't necessarily be caught by today's suite.

### Validation run

- `cargo fmt --check` (workspace): **1 diff** — pre-existing, in `ckan/tests/benchmark_concurrent.rs:36` (a `write_all` call split across lines that `rustfmt` would now collapse). Not introduced this phase; not fixed, per Phase 0's "do not change architecture" instruction.
- `cargo check --workspace --all-targets`: clean, no warnings.
- `cargo clippy --workspace --all-targets`: **2 pre-existing warnings**, both in test files, both style-only (`doc_nested_refdefs` in a doc-comment link in `pipeline_concurrent.rs`, `unnecessary_map_or` in the same file). No warnings in library/binary code.
- `cargo test -p ckan`: **34 passed, 0 failed, 3 ignored** (benchmarks).

### Identified architectural seams

These are the places the plan's later phases will need to cut into — flagged here, not acted on:

1. **No durable work record.** "Pending" is `eligible - installed`, recomputed every run from CKAN + filesystem. Phase 1's `DISCOVERED→QUEUED→RUNNING→PUBLISHED` state needs a new persisted structure; nothing today tracks `attempt`, `worker_id`, `started_at`, or `last_error` for a version.
2. **Failure has no memory across runs.** `failed_this_run` dies with the process. Phase 1/2's "retryable failures eligible for retry" reconciliation rule has no failure history to consult yet — today a failed version is retried unconditionally next run, which is a strictly weaker (but not wrong) version of the target behavior.
3. **Producer and executor are fused.** `pipeline::run`'s `for resource in pending` loop both decides what to run and spawns it, gated only by permit acquisition. Phase 3's bounded-queue-with-backpressure will need to split "what should run" from "spawn it now," since currently there's no queue object to inspect depth on or drain independently of spawning.
4. **Extract and Validate are one function.** `archive::validate_and_extract` interleaves the pre-extract CRC pass, the extraction call, and the post-extract re-check in a single function. Phase 4's explicit stage boundaries (Extract vs. Validate as distinct steps with distinct durable-state-before/after) will need to decide whether to split this or treat it as one atomic stage.
5. **Single concurrency dimension.** One semaphore covers download + extract + convert as one undifferentiated resource. Phase 5's network vs. CPU/disk permit split requires threading two separate `Semaphore`s through `process_version` where today there's one, held for the task's full lifetime.
6. **Recovery granularity is "redo everything."** `clean_staging()` nukes `.staging/` wholesale on every start. Phase 6's stage-aware recovery matrix (e.g., "after conversion → publish existing completed staging" instead of restarting) is a real behavior change from today's blanket wipe, not just an additive one — worth flagging explicitly since it changes what currently happens on a warm restart.
7. **Lock only serializes invocations, not versions.** Phase 7's per-version lease/ownership model is additive on top of `UpdaterLock`, not a replacement — the plan's phase text confirms this, but worth confirming the global lock's purpose (one invocation at a time) doesn't disappear when leases arrive; it's a different axis (invocation-level vs. version-level exclusivity).
8. **`RunSummary` is the only observability surface today** — a `println!`+`tracing::info!` block at the very end of a run. Phase 8's per-stage-duration, per-attempt-traceable logging has no scaffolding yet (no `stage` field anywhere in current `tracing` calls, no `worker_id` concept at all).

No decisions were made about *how* to close these seams — that's Phase 1+ territory.

---

Approved. Proceeded to Phase 1.

---

## Phase 1 — Durable per-version work model

### Implemented

A new control-plane state model, entirely additive — nothing in the existing discover/download/publish pipeline (`pipeline.rs`) was touched or wired to it yet. That wiring is Phase 2's job (the reconciliation scheduler that decides what work is eligible by consulting this state alongside the filesystem).

- **`ckan::work_state`** (new module): the `WorkState` enum (`Discovered`, `Queued`, `Running`, `Published`, `Failed`) and the `VersionWork` record (`version`, `source_url`, `state`, `attempt`, `worker_id`, `started_at`, `completed_at`, `last_error` — exactly the field list the plan specified).
- A pure `is_valid_transition(from, to) -> bool` function encodes the state graph:

  ```text
  DISCOVERED -> QUEUED
  QUEUED     -> RUNNING
  RUNNING    -> PUBLISHED
  RUNNING    -> FAILED
  RUNNING    -> QUEUED     (stale-running recovery)
  FAILED     -> QUEUED     (retry)
  PUBLISHED  -> PUBLISHED  (idempotent no-op)
  ```

  Every other `(from, to)` pair is rejected with `InvalidTransition { from, to }`.
- Higher-level methods (`queue`, `start`, `publish`, `fail`, `retry`, `recover_stale_running`) wrap that graph and additionally maintain the metadata fields: `start` increments `attempt` and clears `last_error`/`completed_at` from any prior attempt; `publish`/`fail` release `worker_id` and stamp `completed_at`; `recover_stale_running` clears `worker_id`/`started_at` but deliberately leaves `last_error` alone, since an interrupted attempt isn't a diagnosed failure.
- `publish()` on an already-`Published` record is a true no-op (returns `Ok(())` without mutating any field) — required for the "version processing is idempotent" invariant to survive a crash between the filesystem's atomic rename and this control-plane update.
- Persistence follows the repository's existing sidecar convention rather than a database: one pretty-printed JSON file per version at `.work/<version>.json`, sibling to the existing `.manifest.json`/`.updater.lock`. Added `RawLayout::work_dir()` / `work_state_path()` in `paths.rs` alongside the other path helpers. `write_work_state`, `scan_work_states` (tolerant of corrupt/non-JSON entries, same pattern as `manifest::scan_sidecars`) round out the module.
- `recover_stale_running(&mut BTreeMap<VersionId, VersionWork>) -> Vec<VersionId>` is a pure, in-memory batch operation: it recovers every `Running` record and leaves everything else untouched. It does not persist anything itself — Phase 2's scheduler will call it during startup reconciliation and then decide when/how to write the result back, alongside the CKAN-discovery merge.

### Files Changed

- `domains/ingestion/extract/ckan/src/work_state.rs` (new) — state machine, persistence, 11 inline unit tests.
- `domains/ingestion/extract/ckan/tests/work_state.rs` (new) — 4 persistence/recovery integration tests.
- `domains/ingestion/extract/ckan/src/paths.rs` — added `work_dir()` and `work_state_path()`.
- `domains/ingestion/extract/ckan/src/lib.rs` — registered `pub mod work_state;`.
- `domains/ingestion/extract/ckan/tests/benchmark_concurrent.rs` — whitespace-only reformat from running `cargo fmt`; this was the one pre-existing `cargo fmt --check` diff flagged in Phase 0, now resolved as a side effect of formatting the new files. No behavior change.

### Tests Added / Updated

All new; nothing existing was modified.

| Test | Proves |
|---|---|
| `happy_path_discovered_to_published` | valid state transitions, attempt/worker_id/timestamp bookkeeping along the way |
| `skipping_queued_is_rejected` | a specific invalid transition (Discovered → Running) is rejected and reports the right `from`/`to` |
| `every_illegal_transition_is_rejected` | exhaustive 5×5 matrix over all states — every pair not in the valid-transition list is rejected, and rejection never mutates `state` |
| `published_cannot_be_restarted` | Published is terminal against Running/Queued/Failed |
| `republishing_an_already_published_record_is_a_noop` | idempotent transition to PUBLISHED — the whole record is byte-for-byte unchanged on a second `publish()` call |
| `a_failed_version_can_be_retried_and_reattempted` | retryable failure — Failed → Queued → Running again, `attempt` goes 1→2, `last_error` clears only once the next attempt actually starts |
| `a_failed_version_stays_failed_until_explicitly_retried` | terminal failure — Failed is a stable resting state; nothing transitions out of it except the explicit retry path |
| `stale_running_recovers_to_queued_without_counting_as_a_failure` | recovery of stale RUNNING — requeues, clears ownership/start time, does **not** touch `attempt` or `last_error` |
| `recover_stale_running_batch_only_touches_running_records` | the batch recovery helper only touches `Running` records among a mixed set of all five states |
| `write_then_scan_round_trips_every_field` | JSON persistence round-trip is lossless |
| `scan_ignores_corrupt_and_non_json_entries` | corrupt/unrelated files in `.work/` don't break a scan (mirrors `manifest::scan_sidecars`'s tolerance) |
| `scan_on_missing_work_dir_returns_empty` | no `.work/` directory yet is a valid empty-state, not an error |
| `stale_running_record_recovers_across_a_restart` | end-to-end: write a crashed Running record + an unrelated Published record to disk, "restart" (scan → recover → persist), reread from disk, confirm only the stale record changed and durably so |

### Validation

- **cargo fmt**: clean (`cargo fmt --check` passes workspace-wide). Running `cargo fmt -p ckan` while formatting the two new files also collapsed the one pre-existing drift in `benchmark_concurrent.rs` flagged in Phase 0 — a formatting-only change, included in the diff above for transparency.
- **cargo check** (`--workspace --all-targets`): clean, no warnings.
- **cargo clippy** (`--workspace --all-targets`): same 2 pre-existing warnings as Phase 0 (both in `pipeline_concurrent.rs`, both style-only), nothing new from `work_state.rs` or its tests.
- **cargo test -p ckan**: **47 passed, 0 failed, 3 ignored** (the pre-existing wall-clock benchmarks). Up from 34 passed in Phase 0 — 13 new tests (9 unit + 4 integration), zero regressions.

### Architectural Notes

- **Control plane vs. data plane, made physically explicit.** `.work/*.json` (control plane: what work should happen) now lives as a sibling directory to the existing `.snapshot-meta.json` sidecars and `.manifest.json` (data plane: what's actually published) under the same `raw_dir` root, rather than a new subsystem — consistent with "follow the repository's existing storage conventions, don't introduce a database."
- **`worker_id` is `Option<String>` and unused for real ownership yet.** As instructed, this phase does not implement leases; `start()` accepts whatever caller-supplied identity is given (or `None`) and stores it purely as a record. Phase 7 is where this becomes a real ownership mechanism with heartbeats and expiry.
- **No retry policy or backoff is encoded here.** `retry()` only asserts the transition is legal; *when* a Failed record should be retried (immediately, after backoff, capped at N attempts) is deliberately left to Phase 2's reconciliation rules, per "do not add abstractions without an immediate use."
- **Recovery is unconditional, not lease-based.** `recover_stale_running` treats every persisted `Running` record as stale, full stop — correct today because `UpdaterLock` guarantees at most one invocation runs at a time, so a `Running` record can only be found at startup if the process that wrote it is gone. This reasoning stops holding once Phase 9/10 allow multiple concurrent workers; at that point stale-detection needs a real lease timeout instead of "found at startup." Flagging this now so it isn't mistaken for an oversight later.
- **Nothing in `pipeline.rs` changed.** The existing discover → download → verify → extract → validate → convert → publish flow, its semaphore-bounded concurrency, its manifest/latest bookkeeping, and its all-or-nothing per-run failure tracking are all completely unchanged and still fully covered by the Phase 0 test suite (all 34 original tests still pass, unmodified). This phase only adds a state model; it does not yet observe or drive real processing.

### Deviations / Risks

- None from the plan's Phase 1 scope. One incidental fix: the pre-existing `cargo fmt` drift in `benchmark_concurrent.rs` (flagged, not fixed, in Phase 0) got swept up by running `cargo fmt -p ckan` on the new files. Called out explicitly in case a whitespace-only diff in an unrelated file is unexpected in this phase's changeset.
- Risk to watch in Phase 2: because `VersionWork` doesn't yet know about `UpstreamResource`'s `name_prefix` (needed to compute `snapshot_dir_name()` for filesystem reconciliation), the reconciler will need either to carry that alongside the control-plane record or recompute it from `source_url`. Not a Phase 1 problem, but worth deciding deliberately rather than bolting on.

### Review Questions

1. Does the `.work/<version>.json` location and naming match what you'd expect for the control-plane store, or would you prefer it nested differently relative to `.manifest.json`/`.updater.lock`?
2. `recover_stale_running` currently has no caller — it's proven correct in isolation but not yet exercised against the real pipeline. Confirm this should wait for Phase 2 rather than being wired into `pipeline::run`'s existing startup sequence now.
3. Any objection to the `worker_id: Option<String>` being freeform (caller-supplied string, no format enforced) at this stage, given Phase 7 will define its real semantics?

---

**Phase 1 review resolved:**

1. `.work/<version>.json` location — approved as-is.
2. `recover_stale_running` remaining uncalled until Phase 2 — confirmed.
3. `worker_id: Option<String>` freeform for now — approved. **Decision carried forward to Phase 7:** when lease-based worker ownership is introduced, `worker_id` should represent a *stable execution identity* — e.g. `hostname+PID`, a UUID, or a worker-instance ID — not an arbitrary label. Recorded here so Phase 7 doesn't have to re-derive this from scratch.

Approved. Proceeded to Phase 2.

---

## Phase 2 — Reconciliation scheduler

### Implemented

A new `ckan::reconcile` module implementing the `discover() → reconcile() → durable work state → eligible work` architecture, built entirely on top of Phase 1's `work_state` module. As in Phase 1, this is additive: `pipeline.rs`'s existing discover/download/publish flow is completely untouched. Wiring the reconciler's `eligible` output into an actual execution path (replacing `pipeline::run`'s direct-spawn loop) is Phase 3's job, per the plan's own architecture diagram (`reconciler → bounded queue → worker tasks`).

`reconcile()` is a single pure function:

```rust
pub fn reconcile(
    resources: &[UpstreamResource],              // from ckan_client::list_gtfs_zip_resources (unchanged)
    cutoff_version: Option<&VersionId>,
    installed: &BTreeMap<VersionId, SnapshotMeta>, // from manifest::scan_sidecars (unchanged) — filesystem authority
    states: BTreeMap<VersionId, VersionWork>,      // from work_state::scan_work_states (Phase 1)
    now: DateTime<Utc>,
) -> ReconcileOutcome
```

It performs no I/O. Callers scan durable state in (`work_state::scan_work_states`, `manifest::scan_sidecars`) and are expected to persist `ReconcileOutcome.states` back out; `ReconcileOutcome.eligible` is the ordered (oldest-first) list of versions now `QUEUED` and ready to be claimed this pass.

Reconciliation rules implemented, in the order they're applied:

1. **Stale RUNNING recovery runs first**, unconditionally, via Phase 1's `work_state::recover_stale_running` — before any upstream resource is even considered. This guarantees no `Running` record can still exist by the time individual versions are reconciled.
2. **Below cutoff → ignored.** No record created; an existing record for an ignored version is left completely untouched (not deleted, not touched at all) rather than assumed irrelevant.
3. **Filesystem-installed → forced to `Published`**, unconditionally, via the new `VersionWork::reconcile_as_published` (see below) — regardless of whatever the control plane currently believes (`Discovered`/`Queued`/`Failed`/no record at all). This is the "filesystem overrides stale control-plane assumptions" rule, and also how a pre-existing snapshot (predating this control plane, or Phase 0's system before Phase 1/2 existed) gets bootstrapped into the model with zero migration step.
4. **No record + not installed → first discovery**: `Discovered` then immediately `Queued` in the same pass, eligible.
5. **Already `Queued` (including just-recovered-from-stale-Running) → stays `Queued`, eligible again.** No duplicate record is created.
6. **`Failed` + not installed → retried**: `Failed → Queued`, eligible. `last_error` is deliberately left in place until the next attempt actually starts (Phase 1's `start()` clears it) — retrying doesn't erase the diagnostic.
7. **`Published` (control plane) but not installed (filesystem) → flagged, not auto-resolved.** Recorded in `diverged_published_without_filesystem` and left completely untouched. This case isn't explicitly named in the plan's rule list, but follows directly from "filesystem is authoritative" colliding with "don't guess" (the same philosophy behind `pipeline::verify_latest_consistency`'s existing fail-loudly behavior): if the control plane says done but the data plane disagrees, the right move is to surface it for investigation, not to silently pick a side.

One new primitive was added to `work_state.rs` to support rule 3: `VersionWork::reconcile_as_published(now)`. Every other Phase 1 method routes through the strict transition graph; this one is a deliberate, documented bypass — it forces `state = Published` from *any* prior state (a no-op if already `Published`), because it represents an outside observation of ground truth overriding the control plane's own bookkeeping, not a normal lifecycle step. It preserves `attempt`/`last_error` (a filesystem observation doesn't get to rewrite this control plane's attempt history) but clears `worker_id` and stamps `completed_at`, matching `publish()`'s semantics for those two fields.

### Files Changed

- `domains/ingestion/extract/ckan/src/reconcile.rs` (new) — `reconcile()`, `ReconcileOutcome`, 12 inline unit tests.
- `domains/ingestion/extract/ckan/tests/reconcile.rs` (new) — 3 end-to-end disk-restart integration tests.
- `domains/ingestion/extract/ckan/src/work_state.rs` — added `reconcile_as_published()` plus 3 inline unit tests for it.
- `domains/ingestion/extract/ckan/src/lib.rs` — registered `pub mod reconcile;`.

`pipeline.rs`, `paths.rs`, and every other Phase 0/1 file are unchanged.

### Tests Added / Updated

18 new tests; nothing existing modified.

| Test | Proves |
|---|---|
| `a_version_with_no_prior_record_is_discovered_and_queued` | first discovery |
| `a_version_already_queued_stays_queued_and_eligible_without_duplication` | already-known versions, no duplicate record |
| `an_already_published_version_with_matching_filesystem_state_is_a_noop` | already-published versions — byte-for-byte unchanged, not just same state |
| `a_failed_version_not_yet_installed_is_retried` | retryable failure, `last_error` preserved until next attempt |
| `a_stale_running_version_is_recovered_to_queued_and_eligible` | stale running work recovered and re-offered |
| `a_version_below_cutoff_is_ignored_and_gets_no_record` | cutoff behavior |
| `cutoff_does_not_disturb_an_existing_record_for_an_old_version` | an ignored version's existing record is left untouched, not deleted |
| `an_installed_filesystem_snapshot_with_no_control_record_bootstraps_as_published` | filesystem overriding a missing control-plane record |
| `an_installed_filesystem_snapshot_overrides_a_failed_control_record` | filesystem overriding a stale `Failed` belief |
| `an_installed_filesystem_snapshot_overrides_a_queued_control_record` | filesystem overriding a stale `Queued` belief |
| `a_published_control_record_without_filesystem_backing_is_flagged_not_requeued` | divergence is surfaced, not silently auto-resolved either way |
| `reconciling_twice_with_the_same_inputs_is_idempotent` | pure-function stability — same inputs, same outcome |
| `reconcile_as_published_forces_state_from_any_non_published_state` (work_state) | the override works from every non-Published state |
| `reconcile_as_published_preserves_attempt_and_last_error_history` (work_state) | the override doesn't rewrite attempt/error history |
| `reconcile_as_published_on_an_already_published_record_is_a_noop` (work_state) | idempotence matches `publish()`'s guarantee |
| `restart_reconstructs_pending_work_purely_from_durable_state` (integration) | end-to-end: run 1 queues+starts a version and "crashes" (leaves it `Running` on disk); run 2 is a fresh scan from disk with no in-memory carryover and correctly recovers + re-offers it |
| `restart_recognizes_a_filesystem_published_snapshot_without_replaying_history` (integration) | a sidecar-only snapshot with zero control-plane history is recognized as `Published` on first scan, never queued |
| `two_passes_separated_by_a_full_persist_and_rescan_agree` (integration) | the disposability property end-to-end through real disk I/O, not just in-memory |

### Validation

- `cargo fmt --check`: clean.
- `cargo check --workspace --all-targets`: clean, no warnings.
- `cargo clippy --workspace --all-targets`: same 2 pre-existing style warnings as Phase 0/1 (both in `pipeline_concurrent.rs`), nothing new.
- `cargo test -p ckan`: **65 passed, 0 failed, 3 ignored** (unchanged benchmarks). Up from 47 in Phase 1 — 18 new tests, zero regressions across all of Phase 0's original 34 and Phase 1's 13.

### Architectural Notes

- **The Phase 1 "seam" about `name_prefix` turned out not to apply.** The Phase 1 report flagged a risk that `VersionWork` would need `UpstreamResource`'s `name_prefix` to compute a snapshot directory name for filesystem reconciliation. It doesn't: `manifest::scan_sidecars` already returns a `BTreeMap<VersionId, SnapshotMeta>` keyed by `VersionId` directly, so "is this version installed" is a plain `installed.contains_key(&version)` check — exactly what `pipeline::run` already does today. `VersionWork` only needs to track control-plane state, never path/directory naming (that stays entirely the data plane's concern, in `SnapshotMeta`/`RawLayout`). Noting the resolution explicitly since the original phase report raised it as an open risk.
- **`reconcile_as_published` is an intentional, narrow widening of the Phase 1 FSM**, not a hole in it. Phase 1's `every_illegal_transition_is_rejected` test still holds for the entire normal lifecycle surface (`queue`/`start`/`publish`/`fail`/`retry`); this new method is a separate, explicitly-named escape hatch used only when reconciling against filesystem ground truth, and is itself fully tested (idempotence, history preservation, works from every prior state). Flagging this transparently rather than letting it look like scope creep on Phase 1's invariants.
- **The divergence case (`Published` control state, no filesystem backing) is new territory the plan's rule list didn't explicitly enumerate.** The chosen behavior — surface via `diverged_published_without_filesystem`, touch nothing — follows the same "fail loudly / don't guess" philosophy as `pipeline::verify_latest_consistency`'s existing `latest`-vs-manifest mismatch handling. An alternative (auto-requeue a diverged `Published` record) was considered and rejected: it would require a `Published → Queued` transition that doesn't exist in the Phase 1 graph, and per DD-001 "manual manipulation of snapshot directories... is not part of normal operation," so this state should be rare and worth a human look rather than silent self-repair.
- **`reconcile()` still has no caller.** Like `recover_stale_running` after Phase 1, it's proven correct in isolation (including two full disk-restart integration tests) but isn't yet invoked from `pipeline::run`. Wiring it in is explicitly deferred to Phase 3, where the reconciler's `eligible` list becomes the producer side of the bounded work queue.

### Deviations / Risks

- None from the plan's Phase 2 scope.
- Carried-forward risk for Phase 3: once `reconcile()` is wired into `pipeline::run`, the existing `pending.sort_by(...)` / semaphore-spawn loop in `pipeline.rs` will need to be replaced by something that consumes `ReconcileOutcome.eligible` and turns each version back into an `UpstreamResource` (or equivalent) for `process_version` — `reconcile()` currently only tracks `VersionId` + `source_url`, not the full resource metadata (`name_prefix`, `upstream_hash`, `publisher_last_modified`) that `process_version` needs. That's fine for Phase 2 (a pure state-tracking exercise) but Phase 3 will need to decide whether the queue carries `VersionId` (looked back up against the discovered resource list) or the full `UpstreamResource`.

### Review Questions

1. The `diverged_published_without_filesystem` handling (surface-only, no auto-action) — confirm this is the right call, or would you prefer reconciliation to force such a record back to `Queued` for reprocessing instead?
2. `reconcile_as_published` bypasses the Phase 1 transition graph by design. Any concern with this pattern (a narrow, explicitly-named override alongside a strict FSM) versus, say, adding `(Any, Published)` as a blanket-legal transition in the graph itself?
3. Confirm Phase 3 should carry `VersionId` through the queue (re-deriving `UpstreamResource` details from the discovery pass as needed) rather than threading the full `UpstreamResource` through `ReconcileOutcome` — the former keeps `work_state`/`reconcile` decoupled from CKAN-specific resource shape, but means Phase 3's queue consumer needs access to the discovery result too.

---

**Phase 2 review resolved:**

1. Approved: keep divergence surfacing behavior, no auto-requeue.
2. Approved: `reconcile_as_published` stays exactly as implemented; no blanket `(Any → Published)` transition added to the FSM.
3. Approved: the queue carries `VersionId` only. **Refinement directed for Phase 3:** do not have the queue consumer re-query CKAN for `UpstreamResource` metadata. The scheduler already performed discovery — that discovery result (the `Vec<UpstreamResource>` from the same pass) must be kept alive in the execution context and looked up by `VersionId` when a worker needs resource metadata, never re-fetched.

Approved. Proceeded to Phase 3.

---

## Phase 3 — Bounded in-process work queue

### Implemented

A new `ckan::queue` module: a generic bounded producer/consumer primitive replacing the "acquire a semaphore permit, then spawn a task" pattern `pipeline::run` uses today. As with Phases 1–2, this is additive and not yet wired into `pipeline.rs` — proving the queue mechanism correct in isolation, with a stub worker, is this phase's job; plugging the real `process_version` stages into it is Phase 4's.

```text
producer (reconciler's eligible work)
    |
    v
bounded channel     capacity = MAX_QUEUED_VERSIONS   (config.max_queued)
    |
    v
fixed worker pool   size     = MAX_ACTIVE_VERSIONS    (config.max_active)
```

`queue::spawn(config, process)` spawns exactly `max_active` long-lived worker tasks up front — never one task per item — sharing one `tokio::sync::mpsc` channel of capacity `max_queued` via an `Arc<Mutex<Receiver>>` (the mutex is held only long enough to pull one item off; processing happens outside it, so many workers process concurrently while at most one is ever mid-`recv`). It returns a `WorkQueue<T>` producer handle, a `Receiver<(T, R)>` of completed results, and the `JoinSet` of worker tasks.

Two independently-bounded knobs, exactly as the plan specifies:

- **`max_queued`** — the channel capacity. Once that many items are buffered waiting for a worker, `WorkQueue::enqueue` blocks. This is the required behavior verbatim: *"If the queue is full: producer blocks/backpressures rather than spawn more tasks."* There is no code path in `queue::spawn` that spawns an additional task in response to load — the worker count is fixed at spawn time.
- **`max_active`** — the fixed worker-pool size. Since each worker processes exactly one item at a time before looping back to `recv`, this directly bounds concurrent processing without a second semaphore.

Graceful shutdown is drain-based, not cancellation-based: `WorkQueue::close()` (or simply dropping the handle) closes the producer side; workers keep processing whatever is already buffered or in-flight and only exit once `recv` reports the channel is both closed and empty. Nothing is aborted mid-item.

Following the Phase 2 refinement, the module is generic over the item and result types (`spawn<T, F, Fut, R>`) rather than hardcoded to a GTFS type — but the module doc comment states plainly that the production item type will be `VersionId` alone, and that a worker needing `UpstreamResource` metadata must look it up from the discovery result already captured in its own closure, never re-query CKAN.

**Documented boundary, found while writing the tests:** this queue is content-blind — it has no identity-level deduplication. Two *separate* `enqueue` calls carrying the same logical `VersionId` are free to run concurrently as far as this module is concerned; single-delivery only guarantees a given *message* goes to one worker, not that a given *identity* is exclusive. Avoiding the latter in production is a producer-discipline property (Phase 2's `reconcile` only ever offers each distinct eligible version once per pass, and a new pass doesn't start until the previous one's queue has drained) and, later, the Phase 4 Claim step's job. Called out explicitly in both the module doc comment and the relevant test, rather than left implicit.

### Files Changed

- `domains/ingestion/extract/ckan/src/queue.rs` (new) — `QueueConfig`, `WorkQueue<T>`, `spawn()`.
- `domains/ingestion/extract/ckan/tests/queue.rs` (new) — 6 integration tests.
- `domains/ingestion/extract/ckan/src/lib.rs` — registered `pub mod queue;`.

`pipeline.rs`, `reconcile.rs`, `work_state.rs`, and everything else from Phases 0–2 are unchanged.

### Tests Added / Updated

6 new tests, covering every bullet in the plan's Phase 3 test list:

| Test | Proves |
|---|---|
| `enqueue_blocks_once_queue_and_workers_are_saturated` | queue capacity — exactly `max_active + max_queued` items are accepted without any completing; the next `enqueue` provably blocks (via a `Notify` gate + a short timeout expected to fail) |
| `blocked_producer_unblocks_and_the_item_is_still_processed` | producer backpressure is temporary, not a deadlock — once a worker frees a slot, the pending `enqueue` completes and that item is still genuinely processed |
| `worker_pool_processes_concurrently_without_exceeding_max_active` | worker consumption — an atomic peak-concurrency counter proves the pool actually reaches `max_active` concurrent processing and never exceeds it |
| `closing_the_queue_drains_remaining_work_before_workers_exit` | queue drain + graceful shutdown — closing the producer mid-flight still delivers every already-queued/in-flight item, and every worker exits only after the drain completes |
| `no_item_is_lost_across_many_more_items_than_queue_capacity` | no lost work — 50 items through a queue of combined capacity 7 (`max_queued=3, max_active=4`); every item delivered exactly once |
| `no_two_workers_ever_process_the_same_enqueued_item_concurrently` | no duplicate active work (as scoped to this module — see "Documented boundary" above) — a shared in-flight `HashSet` with an assert-on-double-insert proves no single enqueued message is ever delivered to two workers at once |

### Validation

- `cargo fmt --check`: clean.
- `cargo check --workspace --all-targets`: clean, no warnings.
- `cargo clippy --workspace --all-targets`: same 2 pre-existing style warnings as Phases 0–2, nothing new.
- `cargo test -p ckan`: **71 passed, 0 failed, 3 ignored** (unchanged benchmarks). Up from 65 in Phase 2 — 6 new tests, zero regressions.

### Architectural Notes

- **A real deadlock was found and fixed during test-writing, not in the queue itself but in two tests' usage pattern.** `no_item_is_lost_across_many_more_items_than_queue_capacity` and `no_two_workers_ever_process_the_same_enqueued_item_concurrently` originally enqueued everything sequentially and only started draining the output channel afterward. With an output channel that's also bounded, and enough items to exceed both channels' capacity, this created a genuine circular wait: workers blocked trying to send completed results into a full, undrained output channel, which meant they couldn't loop back to drain the input channel, which meant the producer's remaining `enqueue` calls never got the room they were waiting for — and the output channel was never going to be drained because the producer's loop, which precedes the drain call in program order, never finished. Confirmed live: `cargo test` hung past 120 seconds, and process inspection showed the `queue` test binary specifically stuck. Fixed by running the producer loop and the output drain concurrently (`tokio::spawn` for the producer, drain in the foreground) in all three multi-item tests, including the one that happened to pass anyway (`closing_the_queue_drains_remaining_work_before_workers_exit`, hardened as a matter of not relying on a timing coincidence). This is flagged prominently because it's a real, reusable lesson for Phase 4: **the eventual `pipeline::run` caller of this queue must drain results concurrently with enqueuing, never enqueue-then-drain sequentially**, or the exact same deadlock reproduces in production. Today's `pipeline::run` already drains concurrently via `JoinSet::join_next` inside the same loop that spawns, so this is a known-safe pattern to carry forward, not a new problem to solve.
- **`max_active` bounds concurrency via worker-pool size, not a second semaphore.** This was a deliberate simplification: rather than layering a `Semaphore` bound on top of unboundedly-spawned tasks (today's `pipeline.rs` approach), the fixed-size pool structurally cannot exceed `max_active` concurrent items — there is no permit to forget to acquire or release. This also sets up Phase 5's resource-specific concurrency cleanly: today one worker does the whole download+extract+convert sequence per item, so pool size alone bounds everything; Phase 5 will need to introduce separate permits *inside* a worker's processing of one item (network vs. CPU/disk), which composes fine with this design without changing the queue itself.
- **The output channel's capacity (`max(max_queued, max_active)`) is an implementation default, not a plan requirement.** It was sized so that, under normal *concurrent* draining (the pattern this queue is meant to be used with), it's unlikely to ever backpressure workers. It is still a bounded channel, consistent with "queue capacity is bounded" as a general invariant, and callers are expected to drain it continuously rather than in a batch at the end (see the deadlock finding above).

### Deviations / Risks

- None from the plan's Phase 3 scope. The deadlock above was caught and fixed within this phase, before merge — not a shipped regression, but documented here in full because it's exactly the kind of subtle backpressure interaction the plan's non-negotiable invariants (bounded queue, bounded resource concurrency) are meant to guard against, and it's worth Phase 4 inheriting the lesson explicitly rather than rediscovering it.
- Carried-forward note for Phase 4: the worker closure passed to `queue::spawn` will need to be constructed with the discovery result (`Vec<UpstreamResource>` or a `HashMap<VersionId, UpstreamResource>` built from it) captured in its closure per the Phase 2 refinement — `pipeline::run` will need to keep that map alive for the queue's lifetime, not just for the initial reconciliation call.

### Review Questions

1. The output-channel-capacity deadlock and its fix (concurrent producer/drain) — confirm this matches your expectation for how Phase 4's `pipeline::run` integration should drain results, or would you prefer a different consumption pattern (e.g., an unbounded output channel, given results are small metadata rather than archive bytes)?
2. `max_active` is enforced purely by fixed worker-pool size (no semaphore). Confirm this is the preferred mechanism to carry into Phase 5, versus reintroducing a semaphore *inside* each worker for the finer-grained network/CPU split.
3. Any concern with the queue module remaining fully generic (`spawn<T, F, Fut, R>`) rather than being specialized to `VersionId` directly, given it's currently only exercised with stub workers and won't be used for anything but `VersionId` in practice?

---

**WAITING FOR APPROVAL** to begin Phase 4 (explicit snapshot processing pipeline).
