# Architecture Review Harness

Evaluate every proposed design against:

## Ownership

- Who owns the data?
- Who owns the event?

## Temporal Correctness

- Is historical reconstruction possible?
- Are timestamps preserved?

## Graph Correctness

- Does the model preserve topology?
- Are graph concepts represented explicitly?

## Operational Impact

- Scalability
- Replayability
- Failure recovery

## ADR Alignment

Does the proposal violate any approved ADR?
