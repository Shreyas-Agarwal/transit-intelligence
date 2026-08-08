# ADR 0013: Adopt Domain-First Workspace Organization

## Status

Approved

## Supersedes

ADR 0010: Adopt Polyglot Runtime Architecture (TypeScript + Python)

---

## Context

ADR 0010 established a two-language split along a workload axis: TypeScript for I/O-bound, information-moving services, Python for CPU-bound, information-reasoning services. That decision was correct for the problem it was solving, but the platform has since outgrown its framing in two distinct ways.

**First, the backend is no longer well-described as "TypeScript for I/O, Python for computation."** Systems-level ingestion work — GTFS-RT stream handling, high-throughput event acquisition, anything latency- or memory-sensitive at the acquisition boundary — has proven to be a better fit for Rust than for Node.js. Rust is being adopted as a first-class backend language alongside Python for the workloads currently in the repository. TypeScript's backend responsibilities under ADR 0010 (realtime gateways, event ingestion services, integration services) are being reassigned to Rust and Python depending on workload shape, for the services that exist today. TypeScript's role is narrowing to frontend for those same services — but this is a statement about current services, not a ceiling on where TypeScript, Rust, Python, or any other language can be used going forward. Which language sits behind the platform's public/internal API surface is explicitly **not decided by this ADR** — Rust, Python, TypeScript/Fastify, Go, Java, or something not yet considered are all live candidates depending on how that service's workload shape settles out, and the decision is deferred until that layer's concurrency and computation profile is clearer.

**Second, and more structurally: the platform is no longer one workload split across two languages. It is multiple independent bounded contexts** — Ingestion, Network Modelling, Operational Analytics, Simulation, Optimization, and future research domains — each of which may use one or more languages internally. ADR 0010's guidance ("is this service moving information or reasoning about it?") answers *which language*, but it has no opinion on *where that language's workspace lives* or *how its dependency graph is scoped*. That silence has a cost: language-ecosystem files (`Cargo.toml`, `pyproject.toml`, `package.json`, and their lockfiles) have been accumulating at the repository root, which implicitly couples every domain that uses a given language to a single shared dependency resolution. Ingestion's Rust and Network Modelling's Rust have no principled reason to share a dependency graph; neither do Ingestion's Python and Simulation's Python. The repository needs an explicit answer to the organizational question, not just the language-selection question — and it is this organizational question, not any specific language roster, that is the actual subject of this ADR.

---

## Decision

The repository adopts **domain-first workspace organization**: bounded contexts are the unit of ownership for language workspaces, dependency graphs, toolchains, and build systems. Language selection remains workload-driven within each domain, but the *organization* of workspaces around domains — not around languages — is the decision this ADR makes durable.

### Guiding Principle

> The repository is organized around bounded contexts. Languages, dependency graphs, toolchains, and build systems are implementation details owned by those domains. No language ecosystem owns the repository.

This is the core architectural commitment and the thing everything else in this ADR derives from. A language is something a domain uses, never something a domain (or the repository) is organized underneath. Concretely, this rules out both a single repo-wide Python workspace and a single repo-wide Rust workspace as default structures — not because those languages are wrong, but because "all Python everywhere" and "all Rust everywhere" are language-first groupings, and language-first groupings are exactly what this ADR rejects as the organizing axis.

### Domain Autonomy

Each bounded context should be independently **buildable, testable, lintable, and deployable using only the manifests contained within that domain.** A domain that requires reaching outside its own directory — into another domain's workspace, or into a repository-root manifest that isn't a pure toolchain/coordination artifact — to run its build or test suite has broken domain autonomy, regardless of how convenient that coupling seemed at the time it was introduced.

This is the practical test for whether the workspace organization is actually working: if you can `cd` into a domain and run its full build/lint/test/deploy cycle without needing anything outside that directory (plus pinned toolchain versions), the boundary is real. If you can't, either the domain has an undeclared dependency on something outside itself, or something that should be a `platform/` package is instead informally shared.

### Language Roster Is Open, Not Fixed

This ADR fixes an *organizational pattern* (domain-first ownership, workload-driven selection within each domain) — not a *closed list of languages*. Specifically:

- Any language mapping described elsewhere in this document reflects languages currently in use for services that currently exist. It is not an exhaustive or permanent set.
- Any domain may propose and adopt a new language for a specific service, when the workload justifies it, without requiring this ADR to be revisited. A domain-local ADR or lighter-weight note documenting the workload rationale is sufficient — this repo-level ADR only needs revisiting if the *organizational pattern itself* changes (e.g. abandoning domain-first ownership), not every time a new language enters a domain.
- A single domain is not required to be single-language. Ingestion is already polyglot internally (Rust + Python + SQLMesh in one bounded context), and this is the expected shape, not an exception. Other domains may end up similarly polyglot as their internal workloads diversify.
- Candidates not currently in the repository — Go, Java, TypeScript/Fastify for a backend service, or anything else — remain admissible wherever workload shape argues for them. Nothing in this ADR should be read as a gate against them.

---

### Workspace Organization

The repository is organized around **bounded contexts (domains)**, not around **languages**. A domain owns every language workspace it needs; language is an implementation detail inside the domain, not an organizing axis across the repository.

This resolves the two competing strategies as follows:

- **Rejected:** a single `pyproject.toml` for all Python in the repo, a single `Cargo.toml` for all Rust in the repo. Centralizing a language's dependency graph at the root couples domains that have no reason to be coupled — a dependency bump in one domain forces re-resolution everywhere that language is used, regardless of relevance.
- **Adopted:** each domain (`ingestion/`, `network-modelling/`, `operational-analytics/`, `simulation/`, `optimization/`, future domains) owns its own workspace per language it uses, with its own manifest and lockfile. Ingestion's Rust workspace, Python workspace, and SQLMesh models are all siblings inside `domains/ingestion/`, because they belong to the same bounded context despite differing implementation languages.

**Cross-domain sharing of the same language** happens through an explicit shared library, not a shared manifest. If two domains both need Python, that is not evidence they should share a `pyproject.toml`; it's a signal to extract a versioned internal package (e.g. `platform/transit-core-py`) that both depend on as an ordinary workspace/path dependency. The platform layer is itself treated as a domain with its own release discipline. This keeps the dependency graph a DAG — `platform → domains`, never `domain → domain` — which bounds coupling risk at O(n) as domains are added, instead of the O(n²) risk of a centralized manifest.

**Root vs. domain — what belongs where:**

Repository root holds only genuine cross-repo invariants:

- `mise.toml` — a repository-level `mise.toml` may provide default toolchain versions. This carries no dependency graph, only version baselines, so it is cheap to share as a default. Domains may override these defaults with their own `mise.toml` where independent evolution of toolchains is justified — e.g. a domain that needs a newer language version ahead of the rest of the repo, or that must pin to an older one for a dependency's sake. As with the Cargo/`uv` workspace exceptions below, this is an override available on justification, not a default to reach for.
- CI orchestration.
- Documentation and ADRs.
- Infrastructure-as-code.
- Optionally, a thin coordination-only workspace manifest (see below) for local developer convenience.

Root does **not** hold `Cargo.toml`, `pyproject.toml`, `package.json`, or any lockfile that resolves dependencies on behalf of more than one domain. Each domain's manifests live inside that domain's directory and are authoritative for that domain's dependency resolution. This is the direct, structural expression of Domain Autonomy above.

**Permitted exceptions, each requiring explicit justification rather than convenience:**

1. A single Cargo workspace may span multiple domains' Rust crates *if* those domains are intended to release in lockstep. If a domain's Rust component later needs an independent release cadence, it is split into its own workspace at that point — not preemptively.
2. A root-level `uv` workspace manifest may reference each domain's `pyproject.toml` as a member purely for CI/local-dev convenience (a single `uv sync` at the top). This does not change ownership: each domain's `pyproject.toml` remains the authoritative, independently-evolvable dependency set. The root manifest is coordination tooling, not a dependency graph.

Illustrative shape:

```
/
  mise.toml
  docs/architecture/adr/
  infra/
  domains/
    ingestion/
      Cargo.toml                  # ingestion's Rust, own workspace
      pyproject.toml + uv.lock    # ingestion's Python, own deps
      sqlmesh/
    network-modelling/
      pyproject.toml + uv.lock    # independent lockfile
    operational-analytics/
      sqlmesh/
      pyproject.toml
    simulation/
      Cargo.toml
      pyproject.toml
    optimization/
      pyproject.toml
  platform/                       # shared code, itself a domain
    transit-core-py/
    transit-core-rs/
  web/
    package.json + pnpm-lock.yaml
```

---

### Language Selection Philosophy (Normative)

Within a domain, language selection is workload-driven, not preference-driven. The deciding question for backend work:

> Is this workload systems-level and I/O-bound, or analytical and computation-bound?

Systems-level, I/O-bound work leans toward a systems language (currently Rust). Analytical, computation-bound work leans toward a language with strong data/scientific tooling (currently Python). Naturally declarative data-transformation work belongs in a declarative modeling layer (currently SQLMesh) regardless of which language owns the surrounding service. This philosophy — workload shape determines language, not the reverse — is the durable part of language selection and is expected to outlive any specific language named in this ADR.

### Current Repository Implementation

The following is the present-day mapping of that philosophy onto actual languages, for the services that exist today. It is implementation state, not architectural decision, and is expected to change as domains evolve — see "Language Roster Is Open, Not Fixed" above.

**Rust**, currently, is used for:

- Systems programming.
- Ingestion services (data acquisition, stream handling).
- Latency- or memory-sensitive I/O boundaries.

**Python**, currently, is used for:

- Analytical computing and data engineering.
- Orchestration.
- Scientific computing.
- Graph processing, temporal analytics, network analysis.
- Batch processing, statistical processing, research and experimentation workflows.

**SQLMesh / SQL**, currently, is used for:

- Declarative data transformations, wherever a domain's pipeline is naturally expressed as models rather than imperative code.

**TypeScript**, currently, is used for:

- The web frontend.

**Undecided:** the language for the platform's API server layer (public/internal APIs, request handling) is deliberately left open. Rust and Python are the most obvious current candidates given what's already in the repository, but TypeScript/Fastify, Go, Java, or another ecosystem entirely are equally admissible if the workload profile argues for them. This is a known gap, not an oversight, and not a two-option decision.

---

### Shared Principles (retained from ADR 0010)

- Service boundaries are defined by responsibility, not by language.
- Events remain language-agnostic.
- Communication occurs through well-defined contracts.
- Redpanda remains the event transport layer.
- PostgreSQL remains the authoritative operational datastore.
- Architectural consistency takes precedence over language uniformity.

---

## Consequences

### Pros

- Domain boundaries and language boundaries are decoupled, which is the correct axis: bounded contexts are the unit of ownership, languages are implementation detail.
- Dependency blast radius is scoped to the domain, not to "everyone using this language."
- New domains can be added without renegotiating a shared root manifest.
- Domain Autonomy gives a concrete, testable definition of "is this organization actually working" (`cd` into a domain, build/test/lint/deploy with nothing outside it).
- Rust gains first-class status for the workloads it's actually winning at (systems/ingestion) as current implementation state, without that status being architecturally frozen.
- TypeScript's current scope is clarified: frontend for today's services, without foreclosing backend use elsewhere later.
- The API-server language question is explicitly deferred rather than forced prematurely, avoiding a decision made with insufficient information.

### Cons

- The number of language ecosystems in active use is not capped by this ADR. Domain-first ownership contains the blast radius of each addition, but it doesn't make additions free; each new language in a domain is still a toolchain, a CI job, and an onboarding cost someone has to justify.
- Domain-local lockfiles mean no single "bump this dependency everywhere" operation; shared upgrades require touching each domain, or extracting to `platform/` if the duplication becomes real cost rather than acceptable divergence.
- Coordination tooling (root-level `uv` workspace, CI) has to actively resist becoming a de facto centralized manifest over time — this requires ongoing discipline, not just an initial correct structure.
- The deferred API-server language decision is an open architectural question that downstream work (auth, contracts) will eventually be blocked on.

---

## Architectural Guidance

**For workspace placement**, the deciding question is:

> Does this code belong to a bounded context, or is it a cross-repo invariant?

If it belongs to a bounded context — even if other domains use the same language — it lives inside that domain's own workspace. If two domains' code in the same language starts converging, extract a shared package under `platform/`; do not merge their manifests. Only toolchain pins, CI, docs, and infra earn a place at the repository root.

**For language selection within a domain**, prefer:

- **Rust** — systems-level, I/O-bound, latency- or memory-sensitive, at the acquisition/ingestion boundary. (Current instantiation of "systems language" — see Language Selection Philosophy.)
- **Python** — analytical, computation-bound, graph/temporal/statistical reasoning, orchestration, research. (Current instantiation of "analytical language.")
- **SQLMesh** — anything naturally declarative as a data transformation, regardless of which language owns the surrounding service.
- **TypeScript** — frontend, for current services. Not excluded from backend use if a future domain's workload argues for it.
- **Undecided** — the API server layer, and any future service where none of the above cleanly fits. Do not default to TypeScript out of habit from ADR 0010, and do not narrow the candidate set to "Rust or Python" out of habit either — both are defaults to resist, not defaults to apply.
- **New languages generally** — admissible per-domain whenever workload shape argues for them; see "Language Roster Is Open, Not Fixed."

**For checking whether a domain boundary is real**, apply Domain Autonomy directly: try building, testing, linting, and deploying the domain using only its own manifests. If that fails without reaching outside the domain (beyond pinned toolchain versions), the boundary needs fixing before more code is added on top of it.
