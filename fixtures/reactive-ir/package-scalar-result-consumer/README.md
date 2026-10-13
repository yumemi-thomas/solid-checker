# Single primitive callback completion

Regression fixture for ADR 0262, modeled on `root-callback-return`. The claims
below and in `expected-claims.json` match the observed findings snapshot.

`takeResult`'s hand-stated graph is copied from
`package-callback-result-consumer`: the result call has min:0 and an exact
callableOnly gate with a closed uses census. A runtime primitive cannot enter
that call. `SyntaxClean` and `BuiltinClean` must have no result-dispatch
obligation. `ProducerViolation` must retain SC2001 **violation** for its
guaranteed tracked producer write; closing the optional result call does not
erase the producer body.

`RefusedCompletions`, `ShadowedDate`, `ShadowedMath` and `ReplacedLocalDate`
must gain no new proven violation. Their dynamic, annotated, object-coercing,
local-receiver, computed, optional, unreviewed, constructed, async, aliased,
fallthrough, multiple-return and finally shapes retain result-dispatch
uncertifiability. The throw-only callback supplies no new primitive proof;
the existing empty-return shortcut may already omit its obligation.
`readResult`, `unknownResult` and `callTrackedResult` prevent the new proof
from closing other protocols, an open census or a required result call.

Solid/signals/web stubs, JSX declaration and tsconfig are copied byte for byte
from `root-callback-return` (rc.9). The synthetic reactive-package declaration
and package manifest are copied byte for byte from
`package-callback-result-consumer`; its four selected graphs and their import
records are copied unchanged. These are fixture-owned signatures, not altered
published timer typings. No package installation is needed.

A visible global `Date.now` replacement/escape must be checked in a separate
project run: the conservative project-wide veto would also withhold
`BuiltinClean` if the replacement shared this tsconfig. The root callback
shape does not imply result-use closure or close createPolled's callbacks.

This fixture's hand-stated contract document is counted in the live
`stableMainDocuments` pin of `scripts/package-contract-phase19.test.mjs`.
