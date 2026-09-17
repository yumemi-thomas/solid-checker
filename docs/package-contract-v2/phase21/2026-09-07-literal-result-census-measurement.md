# Local literal results: full-corpus measurement

ADR 0053 closes **six more candidate occurrences**, taking the full corpus
from **468 to 462 withheld candidates**. The remaining inventory is **442
creates** (405 census refusals and 37 uncompleted vetoes), plus **20 returns**.
Rows remain **368 certified / 30 refused**, with 20 of the 418 probes not
advanced to certification.

All six closures are `motion-dom@12.43.0`'s
`scrapeSVGMotionValuesFromProps` under the motion-solidjs root. Its HTML
helper returns the same `newValues = {}` binding on both completion paths.
Protocol 36 states the exact call, helper, allocation and return sites; the
consumer binds that result identity to the helper's complete execution
census. Both the later computed write and read use the explicit allocation
premise. The complete blocker set closes, not just its first recorded form.

Every closure is explicitly present in an accepted main as closed empty
creates, and each main's SHA-256 matches its receipt's mainDigest. There are
**zero added withholdings and zero changed remaining first refusals** after
normalizing temporary source roots. All 418 probe IDs, installedVersions
fields and certification statuses match the preceding full run.

[The evidence JSON](2026-09-07-literal-result-census-measurement.json) preserves
all 462 remaining occurrences, every refused/unattempted row, all audit paths
and hashes, the six removals and their receipt-bound mains, and the raw-report,
checker and producer identities. It is the exhaustive current inventory.

## Remaining work

B's original 24 first blockers now number **18**: the recursive/mixed context
result, array/host selector result, and dynamic parser result families remain
outside this one-allocation derivation. No remaining B case changed reason.

E remains **52** returned-callable first refusals. The next bounded target is
the `checkStringStartsWith` factory behind `isCSSVariableName`: it returns one
arrow and captures a literal string argument. The proof must bind the exact
initializer, factory, returned body and captured environment; merely finding
an arrow within the factory cannot authorize arbitrary factory results.
Other E families include nested factories, mutable memoization state and
captured callback invocations, and must keep their independent blockers.

All other counts and required premises remain as stated in the
[preceding exhaustive ranking](2026-09-07-dependency-census-measurement.md).
The active goal continues: these are checker/fact limitations to work on,
not package defects or a declaration that the task is complete.

## Focused verification

The producer regression passed, with explicit form counts preventing vacuous
negative fixtures. The native packed-archive test passed the complete-return
case and four refusal controls. Consumer binding tests passed exact evidence
and rejected altered call/callee/allocation/return locations, an empty return
set, missing helper evidence and a contradictory subject derivation.
All-target Clippy caught and prompted updating the test-only wire-struct
literal immediately; the subsequent all-target run passed. Full `make verify`
passed with exit 0, TOTAL **231.78 seconds**, and no failure marker, including
armed process tests, Go race tests, coverage, ownership, contract corpus and
the TypeScript oracle.

No snapshot or public package-contract artifact was changed for this slice.
Nothing was committed or pushed.
