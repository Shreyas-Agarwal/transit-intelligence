# ADR 0010: Adopt Polyglot Runtime Architecture (TypeScript + Python)

## Status

Approved

## Supersedes

ADR 0002: Adopt TypeScript Across Stack

---

## Context

The original architecture standardized on TypeScript across the frontend, backend, and supporting services, with Python reserved primarily for analytics and machine learning workloads.

As the Transit Intelligence platform evolved, it became clear that the system contains fundamentally different classes of workloads:

1. High-concurrency I/O-bound services.
2. Data-intensive analytical and computational services.

Attempting to force both workload classes into a single language creates unnecessary complexity and prevents the project from leveraging the strengths of mature ecosystems.

The platform's long-term goals include:

* GTFS-RT ingestion.
* Event-driven processing.
* Graph construction and analysis.
* Temporal network reconstruction.
* Accessibility and routing analytics.
* Historical performance analysis.

These workloads benefit from different technology stacks.

---

## Decision

The platform adopts a polyglot runtime architecture.

Both TypeScript and Python are considered first-class implementation languages.

Language selection must be driven by workload characteristics rather than organizational preference.

### TypeScript Responsibilities

TypeScript is the primary language for:

* Frontend applications.
* Public APIs.
* Internal APIs.
* Realtime gateways.
* Event ingestion services.
* Event publishing services.
* WebSocket services.
* Integration services.
* Infrastructure orchestration utilities.

TypeScript services should primarily own:

* I/O-heavy workloads.
* Network communication.
* Request handling.
* Event acquisition.
* Event distribution.

Primary runtime:

* Node.js

Primary backend framework:

* Fastify

---

### Python Responsibilities

Python is the primary language for:

* Graph processing.
* Temporal analytics.
* Network analysis.
* Data science workloads.
* Batch processing.
* Event consumption and enrichment.
* Research and experimentation workflows.

Python services should primarily own:

* CPU-intensive workloads.
* Data transformation.
* Statistical processing.
* Algorithmic analysis.
* Transit intelligence generation.

---

### Shared Principles

Regardless of language:

* Service boundaries are defined by responsibility.
* Events remain language-agnostic.
* Communication occurs through well-defined contracts.
* Redpanda acts as the event transport layer.
* PostgreSQL remains the authoritative operational datastore.
* Architectural consistency takes precedence over language uniformity.

---

## Consequences

### Pros

* Each workload uses the most appropriate ecosystem.
* Improved performance characteristics for both I/O and analytical workloads.
* Access to mature Python graph and data-processing libraries.
* Access to mature Node.js networking and integration tooling.
* Clear separation between acquisition systems and intelligence systems.

### Cons

* Multiple language toolchains must be maintained.
* Developers and AI agents must understand two runtime ecosystems.
* Additional operational complexity compared to a single-language architecture.
* Cross-language contracts require stronger discipline.

---

## Architectural Guidance

When selecting an implementation language, prefer:

### TypeScript

If the primary concern is:

* Network I/O
* Request throughput
* API development
* Event acquisition
* Event publishing
* Realtime communication

### Python

If the primary concern is:

* Computation
* Analytics
* Graph algorithms
* Temporal reconstruction
* Machine learning
* Data processing

If uncertainty exists, the deciding question should be:

> Is this service primarily moving information, or reasoning about information?

Services that move information should generally be implemented in TypeScript.

Services that reason about information should generally be implemented in Python.
