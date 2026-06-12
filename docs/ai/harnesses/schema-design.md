# Schema Design Harness

Before creating a schema:

1. What business concept does it represent?
2. Is it a fact, event, state, or projection?
3. Does it contain temporal information?
4. Can historical state be reconstructed?
5. Does it duplicate existing ownership?

Prefer:

* Explicit keys
* Immutable events
* Clear ownership

Avoid:

* Catch-all tables
* Generic JSON blobs
* Hidden temporal semantics
