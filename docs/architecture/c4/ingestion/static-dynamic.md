# C4 — Dynamic: One Check-and-Update Run

Mermaid has no native C4 "Dynamic" diagram type, so this is a sequence diagram — the standard stand-in for a C4
dynamic view. It traces one invocation of `ckan::pipeline::run` in the order the code actually executes.

Participants are deliberately collapsed to the ones that matter for *interaction order*: `Worker` represents
whichever module a step actually runs in (`snapshot`, `download`, `archive`, `parquet_convert` — see
[static-component.md](static-component.md) for the module-level wiring); `Upstream` collapses the CKAN API and the
resource host into one lane, since both are the same real-world publisher and the distinction doesn't matter to
*this* diagram's story. Only **one** worker's stage sequence is drawn in full — the `par` block makes clear that
this repeats, concurrently, for every queued version, bounded by the worker pool and the two resource-specific
permit pools, rather than trying to draw N workers side by side.

```mermaid
sequenceDiagram
    actor Scheduler
    participant CLI as ckan (main)
    participant Pipeline
    participant Queue as Bounded Queue
    participant Worker as Queue Worker (snapshot)
    participant Upstream as opentransportdata.swiss
    participant Raw as Raw Snapshot Store

    Scheduler->>CLI: invoke
    CLI->>Pipeline: run(layout, ckan_client, download_http, concurrency)

    rect rgba(200, 200, 200, 0.1)
    Note over Pipeline,Raw: Startup — always runs, before any network call
    Pipeline->>Raw: acquire updater lock (stale-PID retried once)
    Pipeline->>Raw: sweep only unresumable *.zip.part staging files
    Pipeline->>Raw: scan sidecars -> installed versions
    Pipeline->>Raw: rebuild + write manifest; verify latest agrees with it
    Pipeline->>Raw: scan durable work state (.work/*.json)
    end

    Pipeline->>Upstream: list_gtfs_zip_resources (package_show, Bearer token, retried)
    Upstream-->>Pipeline: [UpstreamResource]

    Note over Pipeline: reconcile() — pure, no I/O:<br/>upstream + work state + installed -> eligible QUEUED versions
    Pipeline->>Raw: persist reconciled work state

    Pipeline->>Queue: spawn fixed worker pool (size: max_concurrent_versions)

    par enqueue eligible versions (blocks if the queue is full)
        Pipeline->>Queue: enqueue(version)
    and drain results as workers finish (must run concurrently — see note below)
        Queue-->>Worker: pull next version
        activate Worker

        Worker->>Worker: Claim — QUEUED to RUNNING, persisted
        Worker->>Raw: inspect staging for resumable progress

        alt no valid archive on disk
            Worker->>Upstream: stream zip to staging
        else valid archive already staged
            Worker->>Raw: re-verify from disk (no re-download)
        end
        Worker->>Worker: Verify — hash against publisher's

        alt extraction not already valid
            Worker->>Worker: Extract + validate (Tier 1)
        else already valid
            Worker->>Worker: skip — reuse existing extraction
        end

        alt conversion not already complete
            Worker->>Worker: Convert CSV to Parquet
        else already complete
            Worker->>Worker: skip — reuse existing conversion
        end

        Worker->>Raw: atomic rename staging -> final; write sidecar
        Worker->>Worker: Complete — RUNNING to PUBLISHED (or FAILED on any step above)
        Worker-->>Queue: result
        deactivate Worker
    end

    Note over Pipeline,Raw: Only after every result is drained
    Pipeline->>Pipeline: newest = max(installed, status verified)
    alt newest differs from current latest
        Pipeline->>Raw: advance latest (atomic symlink swap)
    end
    Pipeline->>Raw: rebuild + write final manifest
    Pipeline->>Raw: release lock (on drop)
    Pipeline-->>CLI: Ok / Err
```

## Notes

* **The `par` block is a correctness requirement, not a diagramming convenience.** Enqueuing every eligible
  version and only then draining results can deadlock: with two independently-bounded channels (the work queue and
  its result channel), workers can get stuck handing back results with nowhere to put them, which stops them
  freeing queue capacity, which stops the producer finishing, which is the only thing that would let draining
  start. Production code runs the producer and the drain loop as genuinely concurrent tasks for exactly this
  reason.
* **Any one stage failing ends that version at `Complete: RUNNING to FAILED`**, not shown as a separate branch per
  stage to keep the diagram readable — Download, Verify, Extract, and Convert each have their own failure path in
  the code (cleaning up exactly what that stage produced), but all of them converge on the same durable outcome:
  the version is marked `FAILED`, nothing partial is ever published, and the next run's `reconcile()` will retry it.
* **The three `alt` blocks inside the worker's sequence are stage-aware resume**, decided once per version from
  what's actually found on disk — not from anything a previous process remembered. A version resuming after a
  crash can skip straight to whichever step its durable staging evidence actually supports.
* The startup block runs on **every** invocation, not just after a crash — this is what makes recovery self-healing
  rather than needing a special "recovery mode." Unlike V1, it no longer wipes all of `.staging/` — only files that
  are unconditionally unresumable are swept; everything else is left for each version's own stage-aware resume
  check.
* `latest` only ever advances after every queued version has been drained — never mid-run, and never backwards.
