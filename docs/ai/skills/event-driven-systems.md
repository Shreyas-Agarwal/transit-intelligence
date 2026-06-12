# Event-Driven Systems

## Core Principle

Events are immutable.

Events are never updated.

Corrections create new events.

## Event Lifecycle

Producer → Stream → Consumer → Materialized State

## Redpanda Usage

Redpanda acts as:

- Immutable event ledger
- Replay source
- Recovery source
- Inter-service transport

It is not a cache.

It is not a database replacement.

## Design Rules

Prefer:

- Append-only events
- Idempotent consumers
- Replayable processing

Avoid:

- Mutable event history
- Consumer-specific event formats
- Tight producer-consumer coupling
