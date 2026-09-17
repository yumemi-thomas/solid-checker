# Exact returned source positions before an array spread

While investigating the returned-value premise needed by Flux Store, the
producer's existing array tracer was found to assign syntax indexes to values
after a spread. In `[...tail, callback]`, syntax index 1 is not a runtime tuple
index: the callback's index depends on the iterable. The native returned
callable proof consumes these paths as exact identity evidence.

Stop tracing an array at its first spread. Preserve positively known prefix
positions, including elisions. A spread in a nested array stops only that
array's traversal; an independently indexed outer sibling remains addressable.
Even `...[]` stays open because this slice adds no iterable cardinality proof.
An empty source list is not evidence that the returned value has no callable.

This narrows facts within the existing protocol, without changing transport,
receipt or certification interfaces. It adds no checker diagnostic and no
new package permission. Regression cases cover fixed positions, holes,
shifted suffixes, a retained prefix, nested spreads and an empty spread.

This is a prerequisite soundness correction discovered during coverage work,
not a newly certified entrypoint or a metric correction. Flux Store still
needs exact dependency-bound returned-store evidence; object-property tracing
alone cannot supply that premise.

Validation: the new test failed in four spread cases before the patch
(`/private/tmp/return-source-spread-before.log`). All six cases and neighboring
return-source regressions pass after it in 0.139 seconds. Full pinned
`make verify` exits 0 with TOTAL 140.59 seconds and no `FAILED during step`
marker (`/private/tmp/return-source-spread-verify.log`), including the rebuilt
producer, armed process tests and the 96-fixture contract corpus. No existing
snapshot was changed. The full ecosystem corpus was not repeated; the previous
326-complete-row result remains a measurement of the archived preceding build,
not an assertion of preservation under this later narrowing.
