# Sprint 04 Retrospective

## What We Planned

Sprint 04 was scoped as a focused, five-objective, seven-day sprint: analytical profiling of the Zurich GTFS subset, a documented semantic model, a network representation ADR, a graph construction pipeline, and baseline graph metrics.

It was explicitly framed as *not* advanced graph analytics — the goal was foundational: get from "a collection of transit tables" to "a formally modeled transit network" before anything more ambitious was attempted.

None of that work happened this sprint. What happened instead turned out to matter more.

---

## What Actually Happened

The sprint spec was committed. Two small, unrelated commits followed over the next eight days. Then the branch went quiet for 39 days.

When work resumed, it wasn't network modeling. It was a from-scratch rebuild of the ingestion layer: a Rust GTFS-static downloader, a Rust GTFS-realtime pipeline on Redpanda, a domain-first workspace reorganization (ADR 0013), a full repository governance and linting stack, and a decommissioning of the original TypeScript prototype. This branch continued that thread — splitting the ingestion domain into `extract`/`transform`, building a Bronze → Silver transform pipeline, and retiring the now-superseded `gtfs_s` domain.

None of it maps to a Sprint 04 objective. It maps to something bigger than any single sprint objective: this is the point where Transit Intelligence stopped being a prototype and started being built like a platform.

---

## Most Important Discovery

**The project outgrew its own sprint plan mid-sprint, and the work that happened instead was the more important thing to do.**

Until this point, Transit Intelligence had largely been a research and exploration effort — GTFS subsetting scripts, a browser-based analytics prototype, notebook-driven investigation. That mode of working is genuinely valuable and shouldn't be looked down on; it's exactly how Sprint 01 through Sprint 03 discovered what was worth building next. But it doesn't scale into something other people can depend on. A realtime ingestion path that only exists as a TypeScript prototype, a repository with no secret scanning or commit hygiene, a workspace where every domain shares one dependency graph — none of that survives contact with a platform that's meant to run continuously and be trusted.

What shipped in this window is the foundation that makes the *next* sprint's network-modeling work durable instead of another one-off notebook exercise: a real ingestion pipeline producing versioned Bronze/Silver snapshots, a domain-scoped architecture that can grow without root-level dependency collisions, and governance tooling that catches problems before they ship. That's not a detour from the roadmap. It's the roadmap growing up.

---

## Unexpected Outcome

Sprint 03 unintentionally produced a second product — Analytics Studio, the browser-native analytical workbench that grew out of the network-explorer prototype. That retrospective flagged it as a side product to be preserved but not over-invested in unless it earned its own justification.

It didn't earn one, at least not inside this repository. `apps/network-explorer` and its supporting `platform/` libraries have been removed from Transit Intelligence entirely and now live as their own separate project called `Stratum`. They were a valid and interesting outcome of Sprint 03's experimentation, but they never had anything to do with transit network modeling, ingestion, or any objective on this roadmap — keeping them here was scope creep, however useful the creep turned out to be. Spinning them out is the right call, and it's a good sign: the project is now willing to say "this is a different thing" and give it its own home, rather than letting an unrelated prototype keep living inside the platform's main repository indefinitely.

---

## What Went Well

### The foundation that shipped is exactly the right foundation

The Rust ingestion domain (extract + transform), the CKAN downloader, the GTFS-RT realtime pipeline, the domain-first workspace reorganization, and the governance stack (secret scanning, commit conventions, CI hardening) are all coherent, well-documented, and load-bearing for everything Transit Intelligence does next. This is infrastructure a serious platform needs and didn't have before this window.

### Sprint 03's scope creep was resolved cleanly

Rather than letting Analytics Studio linger as permanent unplanned scope, it was cut loose into its own repository. That's the healthy version of what Sprint 03's retrospective asked for: "future investment should occur only when it directly supports Transit Intelligence objectives or when a separate product strategy becomes justified." A separate product strategy became justified, and the separation actually happened.

### The maturity shift is real and worth naming

Up to this point, the project could reasonably be described as a fun exploration: subset a GTFS feed, poke at it in a notebook, build a dashboard, see what's interesting. What shipped in this window — durable ingestion, versioned snapshots, validation gates, secret scanning, a domain-scoped build system — is the difference between a project and a platform. That's a meaningful milestone, even though it wasn't the milestone Sprint 04 was written to hit.

---

## What Could Be Tightened

### The sprint plan should have been updated when the priority shifted

Whatever reasoning led from "build the network model" to "the ingestion layer needs to be rebuilt first" was sound in hindsight, but it happened entirely outside the documentation trail — no spec revision, no note, no linked decision. A reader of the roadmap in July would have had no way to know Sprint 04's plan was no longer what was being worked on. Next time a sprint's priority shifts this significantly, updating the spec (or opening a new one) same-week keeps the roadmap trustworthy without slowing anyone down.

### 39 days without a commit is worth a check-in, whatever the reason

Seven planned days became 49 actual days, with a long gap in the middle. If the branch is quiet because something more important is being figured out, saying so briefly — even a one-line note — keeps the roadmap useful as a record rather than something read after the fact through `git log`.

### The reserved ADR number got claimed by something else

Sprint 04's spec reserved ADR 0013 for the network representation decision. It ended up used for the workspace reorganization instead, since that decision needed a number and 0013 was next in line. Small thing, easy fix: Sprint 05 renumbers the network representation ADR to 0014. Checking open sprint specs before claiming the next ADR number is a cheap habit that avoids this going forward.

---

## Lessons Learned

### Platforms get built by pivots like this one

The most valuable thing that happened this sprint wasn't on the plan, and that's fine — arguably it's the point. A roadmap should describe intent, not act as a cage. The lesson isn't "don't deviate from sprint specs." It's "when you deviate for a good reason, say so in the same place the plan lives," so the roadmap keeps telling the truth about what's actually happening.

### Scope creep isn't a problem — unexamined scope creep is

Analytics Studio was good scope creep in Sprint 03 and the right call to spin out in Sprint 04's window. The pattern to keep is: let interesting tangents happen, then make a deliberate, visible call about whether they belong in this repository or their own. Don't just let them accumulate by default.

### This was a one-off, and it should stay one

This retrospective is being written after the fact because the deviation was large enough to need explaining, not because deviating from a spec is inherently bad. If every future sprint needs a retrospective like this one, sprint specs have stopped meaning anything. The next few sprints should look like Sprint 03's — planned work delivered, any deviation explained in real time, in the spec itself, as it happens.

---

## Follow-Up Actions

* Open a proper spec for the ingestion rebuild, retroactively, so this substantial and valuable piece of work has its own honest record instead of borrowing Sprint 04's.
* Carry Sprint 04's five original objectives into Sprint 05 (`docs/roadmap/phase-1/sprint-05/sprint-spec.md`), now that the ingestion foundation they depend on actually exists — arguably a better position to attempt them from than the one Sprint 04 started in.
* Renumber the network representation ADR to 0014 in Sprint 05, and confirm it's still free before writing it.
* Note in the roadmap that `apps/network-explorer` / Analytics Studio has been spun out to its own repository and is no longer part of Transit Intelligence's scope, so future readers don't go looking for it here.
* When a sprint's priority shifts mid-flight going forward, update or replace its spec in the same week — not 49 days and one retrospective later.

---

## Final Assessment

Sprint 04 did not deliver its stated objectives. It delivered something that wasn't on the plan and turned out to matter more: the ingestion and governance foundation that makes Transit Intelligence a platform other work can depend on, plus a clean separation of the Sprint 03 prototype that had outgrown its place in this repository.

This is being logged as an unplanned but justified pivot — worth calling out clearly precisely because of how significant it is, not because it reflects poorly on the sprint. The ask going forward is simple: when the next pivot like this happens, and it probably will, write it into the spec as it happens rather than reconstructing it afterward from commit history.
