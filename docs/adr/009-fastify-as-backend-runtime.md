# ADR 0009: Standardize Backend Runtime on Fastify Instead of Express or NestJS

## Status

Approved

## Context

The Transit Intelligence Platform requires a backend runtime capable of supporting:

- high-frequency operational telemetry ingestion
- replayable temporal state APIs
- event-driven processing orchestration
- lightweight gateway responsibilities
- low-overhead request handling
- future streaming and observability integrations
- modular architectural boundaries
- HTTP/2 and gRPC communication layers
- columnar analytical data access patterns

The initial project scaffolding and repository bootstrapping were implemented using Express.js because:

- ecosystem familiarity is high
- setup friction is minimal
- rapid prototyping velocity was prioritized during the earliest repository initialization phase

However, as the platform architecture matured, the project evolved away from a conventional CRUD-oriented web application into a computational observability platform with:

- temporal graph processing
- replayable operational state reconstruction
- event-driven ingestion
- streaming infrastructure
- analytical workers
- long-lived operational pipelines
- columnar analytical processing
- protobuf-driven service communication
- replay-based temporal analytics

This changed the backend runtime requirements significantly.

The backend is no longer expected to behave primarily as:

- a monolithic MVC application
  or
- a business CRUD orchestration server.

Instead, the backend increasingly acts as:

- an orchestration and observability layer
- a lightweight operational API gateway
- a temporal state access layer
- a coordination boundary between ingestion, analytics, and visualization systems

The project therefore reevaluated:

- Express.js
- NestJS
- Fastify

before committing to the long-term backend runtime architecture.

---

## Decision

The platform will standardize on Fastify as the long-term backend runtime framework.

The current Express scaffolding will be progressively migrated to Fastify as Phase 1 infrastructure stabilizes.

Express remains temporarily acceptable during the bootstrapping stage, but it is not considered the long-term architectural target.

---

## Why Fastify Was Chosen

Low Overhead Runtime Architecture

Fastify provides significantly lower request lifecycle overhead compared to Express due to:

- optimized serialization
- schema-driven request handling
- efficient plugin encapsulation
- reduced middleware chaining overhead

This aligns better with:

- telemetry-heavy APIs
- operational observability workloads
- replayable state queries
- streaming-oriented infrastructure

The project prioritizes operational efficiency and architectural control over maximal middleware ecosystem flexibility.

---

### Native HTTP/2 Alignment

The platform architecture already incorporates:

- HTTP/2 transport assumptions
- gRPC-compatible communication pathways
- protobuf-based contracts

Fastify provides significantly better alignment with modern HTTP/2-first infrastructure compared to Express.

This is important because future internal communication layers are expected to increasingly rely on:

- multiplexed transport
- binary serialization
- protobuf contracts
- low-overhead service communication

Fastify’s architecture integrates more naturally with:

- HTTP/2
- gRPC gateway patterns
- streaming-compatible operational APIs

without requiring excessive framework-level adaptation.

---

### Better Compatibility With gRPC & Protobuf-Oriented Systems

The platform architecture already standardizes around:

- protobuf contracts
- event-driven schemas
- binary transport semantics
- replayable operational payloads

Fastify’s lightweight request lifecycle and schema-driven model align more naturally with:

- protobuf serialization
- gRPC interoperability
- low-overhead internal service communication

This is particularly important because:

- replay systems
- temporal observability APIs
- propagation engines
- future simulation systems

benefit substantially from efficient binary transport layers rather than JSON-heavy REST semantics.

---

### Strong Alignment With Columnar Analytical Architectures

The platform increasingly operates on:

- append-heavy telemetry pipelines
- analytical snapshots
- replayable temporal datasets
- column-oriented aggregations

Phase 1 already incorporates:

- DuckDB

and later phases may introduce:

- ClickHouse

Both systems follow Apache-style columnar analytical processing philosophies optimized for:

- vectorized execution
- analytical scans
- temporal aggregations
- append-oriented telemetry workloads

Fastify aligns particularly well with this architecture because:

- it minimizes serialization overhead
- it supports efficient streaming responses
- it avoids unnecessary middleware layers
- it performs well under analytical API workloads

The backend increasingly acts as an orchestration boundary around columnar analytics engines rather than as a traditional relational CRUD application server.

---

### Strong TypeScript Alignment

Fastify integrates cleanly with modern TypeScript-first workflows and supports:

- typed schemas
- typed request/response contracts
- strongly typed plugins
- predictable encapsulation boundaries

This aligns closely with the project’s emphasis on:

- type rigidity
- agent-assisted code verification
- contract stability
- replay consistency

---

### Plugin Encapsulation Model

Fastify’s plugin architecture naturally supports:

- modular subsystem isolation
- bounded operational contexts
- incremental capability composition

This maps well onto the platform’s modular monolith architecture.

Examples:

- ingestion plugins
- observability plugins
- replay APIs
- analytics endpoints
- operational metric exposure

The encapsulation model reduces accidental global state leakage and uncontrolled middleware mutation.

---

### Better Long-Term Observability Alignment

The platform’s architecture increasingly resembles:

- event-driven infrastructure
- operational telemetry systems
- streaming observability platforms

rather than:

- traditional REST-heavy CRUD SaaS systems.

Fastify aligns more naturally with:

- lightweight orchestration
- structured logging
- OpenTelemetry instrumentation
- schema-driven APIs
- high-throughput operational endpoints

---

### Better Solo-Developer Operational Simplicity

The project intentionally avoids:

- unnecessary abstraction layers
- excessive decorators
- framework-heavy inversion-of-control systems
- enterprise ceremony

Fastify provides:

- direct architectural control
- simpler runtime semantics
- lower conceptual overhead
- easier debugging and profiling

This is important for a long-horizon solo-operated systems project.

---

## Why Express Was Not Retained

Express remains:

- stable
- battle-tested
- ecosystem-rich

and was useful for initial scaffolding and rapid bootstrapping.

However, the platform ultimately chose not to standardize on Express because:

---

### Middleware-Centric Architecture

Express relies heavily on unconstrained middleware composition, which increases the risk of:

- hidden runtime mutations
- architectural drift
- inconsistent request lifecycle behavior

This becomes increasingly problematic in:

- event-driven systems
- observability-heavy systems
- replay-sensitive infrastructure

---

### Weak HTTP/2 Alignment

Express fundamentally originated in an HTTP/1.1-first ecosystem.

While HTTP/2 support is possible, it is not deeply aligned with Express’s core architecture.

The platform expects increasing reliance on:

- multiplexed transport
- protobuf communication
- gRPC interoperability
- binary operational protocols

Fastify aligns substantially better with this direction.

---

### Lower Type Safety

Express is fundamentally less opinionated around:

- schema validation
- request typing
- serialization contracts

This increases the likelihood of:

- runtime inconsistencies
- hidden coercions
- weak API contracts

which conflicts with the project’s verification-oriented engineering philosophy.

---

### Performance Ceiling

Although Express performance is sufficient for many systems, the platform expects:

- continuous operational telemetry
- temporal state queries
- replay reconstruction APIs
- high-frequency observability workloads

Fastify provides a cleaner long-term performance profile for this operational model.

---

## Why NestJS Was Not Chosen

NestJS was evaluated seriously because it provides:

- strong modularity
- TypeScript support
- structured architectural patterns
- dependency injection
- enterprise-grade tooling

The project also strongly considered:

- NestJS running on top of Fastify using the Fastify adapter

This would have combined:

- Fastify’s runtime performance characteristics
  with:
- NestJS’s modular organizational structure.

The combination was considered attractive because it offers:

- Fastify-backed transport performance
- strong TypeScript ergonomics
- modular architectural conventions
- dependency injection
- enterprise-grade scaffolding

However, after evaluation, the platform intentionally chose:

- standalone Fastify
  instead of:
- NestJS layered over Fastify.

---

### Runtime Transparency Was Prioritized Over Framework Abstraction

The deciding factor was not raw performance.

The deciding factor was:

- runtime transparency
- architectural observability
- explicit systems understanding
- direct control over execution flow

This project is fundamentally:

- a long-horizon systems engineering initiative
- an operational observability platform
- a computational architecture learning environment
- a personal engineering and systems growth project

A major objective of the platform is not only to build the final system, but also to deeply understand:

- request lifecycles
- plugin boundaries
- transport semantics
- observability pipelines
- event-driven orchestration
- replay systems
- operational infrastructure behavior

Using standalone Fastify preserves:

- explicit runtime visibility
- lower abstraction depth
- clearer performance tracing
- simpler debugging paths
- stronger understanding of underlying execution semantics

NestJS, even when paired with Fastify underneath, still introduces:

- inversion-of-control abstractions
- dependency injection indirection
- framework lifecycle complexity
- reflection-heavy orchestration patterns

While these abstractions are valuable in many enterprise environments, they were considered counterproductive for the goals of this project.

The platform intentionally prioritizes:

- systems understanding
- architectural transparency
- operational visibility
- low hidden magic
- direct control over execution semantics

over:

- enterprise framework convenience
- annotation-driven architecture
- framework-managed orchestration

---

### Excessive Framework Abstraction

Nest introduces substantial framework-level abstraction through:

- decorators
- dependency injection containers
- lifecycle indirection
- reflection-heavy patterns

While useful in large enterprise teams, this abstraction was considered excessive for:

- a systems-oriented observability platform
- a long-horizon solo-operated project

The project prioritizes:

- explicit runtime behavior
- architectural transparency
- low hidden magic

over enterprise framework structure.

---

### Increased Cognitive Overhead

The platform already contains substantial complexity in:

- temporal graph modeling
- replay semantics
- event-driven observability
- operational analytics
- multimodal temporal abstractions

Adding additional framework abstraction was considered counterproductive.

The complexity budget should remain focused on:

- domain modeling
- operational semantics
- computational correctness

rather than framework orchestration semantics.

---

### Reduced Runtime Transparency

Fastify exposes runtime behavior more directly, making:

- profiling
- debugging
- instrumentation
- performance tracing

simpler and more predictable.

This is especially important for:

- event-driven pipelines
- replay systems
- temporal observability infrastructure

where operational visibility is critical.

---

### NestJS Introduces Enterprise Patterns Earlier Than Necessary

NestJS strongly encourages:

- service-container architectures
- enterprise layering conventions
- annotation-heavy composition
- framework-driven organization

The project intentionally avoids premature enterprise-style architectural inflation.

The platform follows:

- modular monolith boundaries
- explicit subsystem isolation
- lightweight orchestration

without requiring a heavy application framework layer.

---

### Additional Runtime Reflection Overhead

NestJS relies extensively on:

- metadata reflection
- decorators
- dependency injection resolution

While often acceptable for conventional SaaS systems, the project prioritizes:

- low-overhead execution
- observability transparency
- predictable operational behavior

especially around:

- telemetry ingestion
- replay APIs
- analytical orchestration layers

---

## Migration Strategy

Current State

The repository currently contains Express-based scaffolding used during the initial bootstrapping phase.

This includes:

- initial API setup
- routing structure
- development server scaffolding

---

## Planned Transition

The platform will progressively migrate toward Fastify during Phase 1 infrastructure stabilization.

Migration priorities:

1. Core API runtime replacement
2. Shared request/response schema standardization
3. HTTP/2 compatibility alignment
4. Protobuf/gRPC transport preparation
5. Plugin encapsulation restructuring
6. Structured logging integration
7. OpenTelemetry readiness
8. Replay API standardization

The migration will occur incrementally to avoid unnecessary disruption during foundational platform development.

---

## Consequences

### Pros

Better Long-Term Performance Characteristics

Fastify provides lower runtime overhead and better scalability characteristics for operational observability workloads.

Stronger HTTP/2 & gRPC Alignment

Fastify integrates more naturally with modern binary transport and multiplexed communication architectures.

Better Compatibility With Columnar Analytical Systems

Fastify aligns well with append-heavy, telemetry-driven analytical architectures built around DuckDB and future ClickHouse integration.

Stronger Type Safety

Schema-driven request handling improves API contract stability and verification quality.

Cleaner Modular Boundaries

Plugin encapsulation aligns well with the platform’s modular monolith architecture.

Better Observability Alignment

Fastify integrates naturally with structured logging, instrumentation, and operational telemetry systems.

Reduced Framework Complexity

The platform retains direct control over runtime semantics without introducing excessive framework indirection.

Better Systems-Level Learning & Architectural Transparency

Standalone Fastify allows deeper understanding of:

- runtime execution
- transport layers
- plugin systems
- observability mechanics
- operational infrastructure behavior

which aligns strongly with the long-term goals of the project.

---

### Cons

Migration Cost

Existing Express scaffolding will require incremental migration work.

Smaller Middleware Ecosystem

Fastify’s ecosystem is smaller compared to Express, requiring more careful dependency evaluation.

Lower Enterprise Familiarity

Some contributors may be more familiar with Express or NestJS patterns.

Additional Architectural Responsibility

Without NestJS-style enforced structure, architectural discipline must be maintained explicitly through:

- ADRs
- module boundaries
- verification guardrails
- governance documentation
- architectural reviews

More Manual Organizational Discipline

Fastify provides fewer framework-enforced architectural conventions compared to NestJS, requiring stronger self-imposed modular discipline.