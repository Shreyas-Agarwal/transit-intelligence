# ADR 0014: Local-First, Durable-State Architecture for the GTFS Static Downloader (V2)

## Status

Accepted

## Date

2026-08-23

## Related

- [DD-001: GTFS Static Auto-Downloader](../../design/DD-001-gtfs-static-downloader.md) — the full technical design this ADR's decision is realized in.
- [IMPL-001: GTFS Static Downloader V2 — Implementation Log](../../implementation/IMPL-001-gtfs-static-downloader-V1.md) — the phase-by-phase record this ADR summarizes the outcome of.

---

## Context

The GTFS Static Downloader (`ckan` crate, `domains/ingestion/extract/ckan/`) started as a straightforward V1: one global lock, one semaphore bounding concurrent version processing, sequential per-version handling, and a recovery strategy of wiping all staging state on every restart. It worked, but it had no durable record of what happened to a version independent of that run's own memory, no way to resume interrupted work below the whole-version granularity, and no way to observe its own behavior beyond ad hoc logging.

An evolution plan was drawn up to address this in 12 explicit phases, each requiring review before the next began: durable per-version state, reconciliation, a bounded work queue, resource-specific concurrency, stage-aware crash recovery, observability, benchmarking, reliability hardening, an architecture review, performance tuning, and finalization.

**The plan's original Phase 7 direction was toward a distributed architecture**: replacing the single global execution lock with per-version worker ownership — leases, heartbeats, stale-lease recovery, explicitly anticipating a future distributed backend (Redis or similar) that could provide `claim()`/`heartbeat()`/`release()` across multiple machines. That direction was reconsidered mid-plan, before any of it was built.

---

## Decision

**V2 is local-first and single-process, by deliberate choice — not a stepping stone left half-finished toward a distributed system.**

The Phase 7 distributed direction was cut by a roadmap revision and replaced with a local-first continuation: observability, end-to-end benchmarking, reliability hardening, an architecture review, performance tuning, and finalization — none of which require, or were blocked by the absence of, distributed worker ownership.

The resulting architecture:

- **Durable per-version state** (`DISCOVERED → QUEUED → RUNNING → PUBLISHED`/`FAILED`), persisted independently of process memory, with reconciliation — a pure function, no I/O — deciding what needs to happen each run from upstream discovery, that durable state, and the filesystem's own installed-snapshot record. The filesystem remains authoritative; a control-plane record that disagrees with it is corrected (if it under-claims) or flagged for investigation (if it over-claims), never silently trusted over what's actually on disk.
- **A bounded local work queue** feeding a fixed worker pool, replacing one global semaphore — backpressure instead of unbounded task spawning, and a clean separation between discovering eligible work and executing it.
- **Resource-specific concurrency**, orthogonal to the worker pool: independent limits for network-bound downloads and CPU/disk-bound extraction+conversion, because a version occupying a worker slot could be doing either kind of work, which puts very different load on the host.
- **Stage-aware crash recovery**: a fresh worker inspects the filesystem for durable, trustworthy evidence of prior progress and resumes from wherever a version actually got to, rather than restarting every stage from scratch on every retry.
- **OpenTelemetry-compatible observability** (spans and metrics via the standard Rust `tracing` ecosystem), so a run's own timing, concurrency, and failure behavior are answerable from what it recorded, not inferred after the fact.

None of this required, or leaves room implied for, per-version worker leases, a distributed queue, or multi-process orchestration. Every reliability, concurrency, and performance question raised while building this evolved architecture — including questions surfaced by real production traces, not just synthetic testing — was answerable inside a single local process. The architecture review conducted partway through this plan (documented in IMPL-001) reached the same conclusion explicitly: the local, single-process design is sufficient for the actual workload.

### Why this was reconsidered, not simply followed as originally planned

The original distributed direction was based on the plan's own initial framing that a downloader eventually operating across multiple machines or a queueing backend was a plausible future need worth designing toward early. Reconsidering it mid-plan, before building any of the leases/heartbeat machinery, rested on:

- **No concrete requirement forced it.** Nothing about GTFS-S's actual publication cadence (twice weekly), the actual data volumes involved, or the actual failure modes encountered demanded coordination across more than one process on one machine.
- **Complexity should follow demonstrated need, not anticipated need.** Leases, heartbeats, and stale-lease recovery are real coordination machinery with real correctness surface area (split-brain, clock skew, lease-expiry races) — worth building when a distributed deployment is an actual requirement, not before.
- **The local architecture remained genuinely evolvable.** Durable per-version state, a bounded queue, and resource-specific concurrency are exactly the primitives a future distributed design would still need underneath it; nothing in the local-first path was thrown away work if that direction is revisited later (see [V3 Considerations](../../design/DD-001-gtfs-static-downloader.md#v3-considerations) in DD-001).

---

## Consequences

### Positive

- The system that got built is simpler than the originally planned trajectory, with no loss of the reliability or observability properties that mattered — those came from durable state, reconciliation, and OpenTelemetry instrumentation, none of which require distribution.
- Every phase after the reconsideration (observability, benchmarking, reliability hardening, tuning) had a smaller, more tractable design space, entirely local and directly testable without simulating multi-machine coordination.
- A real production trace (captured during the performance-tuning phase) was usable directly as evidence, without needing a distributed-tracing setup across multiple hosts.
- The decision is reversible in principle: the durable-state and reconciliation primitives this architecture is built on are the same primitives a distributed design would need, not a design that has to be undone first.

### Negative

- If a genuine multi-machine requirement emerges (see V3 Considerations), coordination machinery (leases, a distributed queue, or equivalent) has to be designed and built at that point — this ADR defers that cost, it doesn't eliminate it.
- The single-process model means the downloader's total throughput is bounded by one machine's network and CPU/disk capacity; there is currently no path to scale beyond that without the deferred distributed work.

---

## Architectural Guidance

For this component and similarly-shaped ingestion components: prefer durable local state and observability over anticipatory distributed coordination. Build toward a distributed architecture only once a concrete requirement (actual multi-machine deployment, actual throughput ceiling reached, actual need for cross-process work ownership) exists — not because a future need seems plausible. When that requirement does arrive, expect it to build on durable per-item state and reconciliation, not around them; that's the reusable part of this architecture, and the part worth preserving in any future domain component built the same way.
