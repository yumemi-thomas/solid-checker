# Shallow callback-result Get

Regression fixture for ADR 0263. The claims in `expected-claims.json` match the
observed findings snapshot.

`readResult` states a closed result-use census with a root Get and immediate
own-value enumeration, both at call on the same stack. `PlainReads` must lose
result-dispatch uncertifiability: evaluating data-property values is producer
work; reading the resulting property does not call those values. Shorthand,
unknown values and nested objects are allowed. The Get has no key field, so
the proof covers any property key, including a missing key, under ADR 0190's
unpatched built-in prototype premise. Transparent TypeScript sugar preserves
the literal identity.

`ProducerViolation` must keep SC2001 violation. `RefusedShapes` must keep
SC9012 uncertifiable for accessors, methods, spreads, computed keys,
`__proto__`, arrays, proxies, dynamic or named returns, async producers and
empty literals (which currently have no ReturnStructureFact).
`OpenOrFurtherUses` keeps SC9012 for an open census, a Get followed by a
property-value call, a queued Get even with undefined factory returns, and
an escaped receiver read at result-access. No new violation is expected.

All Solid/signals/web stubs, the synthetic package's index.d.ts and manifest,
JSX declarations and tsconfig are byte-identical to
`package-scalar-result-consumer`. These are fixture-owned signatures; no real
static-store typing is loosened. The existing real-package ledger records
tsc silence for both derived-store twins. Other contract graph fields and
import records come from `package-callback-result-consumer`; selected uses
are edited only to isolate this consumer boundary.

Its hand-stated contract document is counted in the live
`stableMainDocuments` pin of scripts/package-contract-phase19.test.mjs.
