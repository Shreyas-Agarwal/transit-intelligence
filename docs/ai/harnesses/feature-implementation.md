# Feature Implementation Harness

Before implementing any feature answer:

1. Which ADRs are affected?
2. Which bounded contexts are affected?
3. Does this introduce a dependency?
4. Does this change a public API?
5. Does this alter temporal semantics?
6. Does this alter graph semantics?
7. Does this require schema changes?

Implementation must stop until these questions are answered.

After implementation verify:

* Build passes
* Tests pass
* Documentation updated
* Contracts unchanged or versioned
