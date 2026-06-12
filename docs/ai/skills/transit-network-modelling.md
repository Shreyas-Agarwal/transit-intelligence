# Transit Network Modeling

## Core Principle

The transit network is a graph.

Schedules, vehicles, and realtime updates are observations of activity occurring on that graph.

The graph itself exists independently of:

* Vehicles
* Timetables
* GTFS feeds
* Realtime events

## Graph Components

### Nodes

Nodes represent physical passenger interaction points.

Examples:

* Stop
* Platform
* Station
* Transit hub

### Edges

Edges represent movement possibilities.

Examples:

* Track segment
* Road segment
* Walking transfer
* Platform transfer

## Important Distinction

Do not model the network primarily as relational tables.

Think first in:

* Nodes
* Edges
* Connectivity
* Reachability
* Traversal

Database tables are storage representations of graph concepts.

## Vehicles

Vehicles are not part of the network topology.

Vehicles traverse the network.

The graph remains valid even if every vehicle disappears.

## Common Mistakes

Avoid:

* Treating routes as edges
* Treating schedules as topology
* Treating GTFS as the source of truth for network structure
* Embedding temporal state into graph definitions

Topology and operations are separate concerns.
