# GTFS Domain Rules

## GTFS is an Input

GTFS is not the domain model.

GTFS is an external representation of transit operations.

Internal models may differ.

## GTFS Static

Provides:

- Stops
- Routes
- Trips
- Stop times
- Calendars

## GTFS Realtime

Provides:

- Vehicle positions
- Trip updates
- Service alerts

## Design Rules

Never couple core business logic directly to GTFS structures.

Introduce domain abstractions first.

GTFS should be transformed into internal representations before entering business workflows.
