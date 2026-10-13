# The `reads` census walks calls (ADR 0165)

A package whose exports each propose `reads: []`, certified one export at a time
by the `reads_call_walk_*` tests in
`rust/crates/solid-facts-backend/src/contract_certification.rs`. Nothing here is
reactive on purpose: the census cannot tell an accessor over a plain box from an
accessor over a signal the call created, so calling what a call built refuses
either way. That is the claim under test, and `createCountdown` in
`@solid-primitives/date@3.0.0-next.3` is its real-world instance.

There is no `expected.json` and the fixture is not in `corpus.json`: it drives
only the in-process tests, with rc.9's audited signals serving ADR 0163's
synthesized veto so the census alone decides.

| Export | Verdict | Why |
| --- | --- | --- |
| `callsNothing` | closes | no call; the form census alone decides |
| `callsPureHelper` | closes | same-package helper walked, reads nothing |
| `invokesArgument` | closes | the caller's own argument: the `callbacks` domain's item |
| `callsStandardLibrary` | closes | `Math.max` by standard-library identity (ADR 0149) |
| `returnsReader` | closes | the accessor is returned, not called during the call (ADR 0146) |
| `mutualRecursion` | closes | a cycle of helpers; the back edge adds no read |
| `readsCreatedAccessor` | refuses | `createCountdown`'s shape: builds an accessor, calls it |
| `callsReadingHelper` | refuses | the same read one frame down, in a helper |
| `readingCycle` | refuses | a cycle one of whose frames reads; a back edge does not excuse it |

The dependency half (`probe-recipes/quiet.mjs` is its test-only veto) composes
a dependency export whose own `reads` is closed and empty, and refuses the same
call when that `reads` is open.

No stub of a published package is involved, so the tsc rule has nothing to
duplicate: every signature here is a plain `number` or `() => number` the
checker never compares against a library.
