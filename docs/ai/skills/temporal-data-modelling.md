# Temporal Data Modeling

## Core Principle

Transit intelligence is fundamentally a temporal problem.

Questions are rarely:

"What is the state?"

Questions are usually:

- What was the state at time T?
- What changed between T1 and T2?
- What did we know at T?
- When did this become true?

## Time Types

### Event Time

When something actually occurred.

Example:

A vehicle reached a stop at 08:01:12.

### Processing Time

When the system observed the event.

Example:

The feed was consumed at 08:01:17.

### Snapshot Time

When a state reconstruction was produced.

Example:

Network state as of 08:02:00.

## Preferred Approach

Store immutable events.

Derive state.

Avoid mutating historical facts.

## Design Rule

Whenever creating a model ask:

"Can I reconstruct historical state from this?"

If not, the design is likely incorrect.
