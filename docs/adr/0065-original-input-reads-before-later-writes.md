# Original input reads before later writes

ADR 0064 correctly refuses a binding read after an unconditional replacement,
but its whole-function unwritten premise also refuses earlier genuine reads.
The retained Utils `handleDiffArray` contracts name whole-input reads for slots
0 and 1, not individual `slice` or `includes` calls. Both inputs have a direct
property read in the opening `const` declarations before either assignment.

Protocol 40 adds `initialParameterReads`: the exact implementation parameter
slot and declaration plus one exact property-use location. The producer proves
only a contiguous opening prefix of `const` declarations with simple names and
identifier/static-property initializers. It stops before any other statement,
call, assignment, destructuring, computed property, or control-flow construct.
All parameters must be ordinary, uniquely named, without defaults or rest.
The implementation must be plain, with no nested callable and no mention of
`arguments` or `eval`. These exclusions prevent earlier getter execution from
reaching a closure that can mutate a parameter binding. Later direct assignments
do not retroactively change the value already read. This is not a general
flow-sensitive origin analysis and does not cover mixed origins or aliases.

The Rust client binds each row to exactly one uncaptured, unaliased, reachable
property-use census row and the selected signature's exact declaration. It
rejects duplicate use selections and malformed slots or locations. The verifier
independently validates those identities and records an `initial-parameter-input`
witness. This additional arm answers only an unasserted whole-root recursive
read input, with no composed provenance. Member shape, member callability,
operation occurrence, cardinality and all other demands retain their separate
proofs. Missing rows confer no permission. No receipt or trust format changes.

The producer regression includes before/after-write pairs, two caller inputs,
same-statement assignment, preceding calls, nested writes, defaults, rest,
branches, loops, aliases, arguments, eval, async/generator and computed access.
Client and verifier mutation tests cover exact slots, declarations, use spans,
duplicates and missing/aliased/captured/nonreachable uses. Packed native cases
include an array length read before later slicing and preserve the unconditional
local-replacement negative control. An early whole-root read must not discharge
a later typed-member claim.

The focused Go producer test, packed native/verifier tests (two selected tests,
10.71 seconds), and protocol-client mutation test pass. Full `make verify` passes
with actual exit 0, `TOTAL 242.12s`, and no failed-step marker in
`/private/tmp/initial-reads-verify-final.log`. The first run correctly rejected
stale frozen-schema hash constants; both producer and consumer now bind the
protocol-40 schema's exact SHA-256. A local Go build-cache permission failure was
avoided by using `rust/target/go-cache` through the same pinned Make targets.

The ordinary CLI [negative control](../package-contract-v2/phase21/2026-09-08-initial-read-negative-control.json)
still refuses the unchanged replacement archive with the matching protocol-40
binary, and publishes no new catalog. No fixture snapshots or bundled contracts
changed. The [nine-probe recovery measurement](../package-contract-v2/phase21/2026-09-08-initial-read-recovery.md)
restores seven artifact selections and four complete rows. All 29 pre-regression
selections and their exported claim sets are preserved, including both Motion
Solid 2 probes' `.`, `./m`, and `./v2`. The
[full run](../package-contract-v2/phase21/2026-09-08-initial-read-full-frontier.md)
preserves that control group but loses 30 other selections across ten rows.
There is no corpus-wide preservation claim.

A separate ordinary-CLI [member-boundary probe](../package-contract-v2/phase21/2026-09-08-initial-read-member-boundary.json)
places the replacement object's method outside the exported implementation, so
the implementation itself has no nested callable. It reads the caller's member
in an opening `const`, then replaces the input before calling the local method.
Runtime evidence records one caller-root read and zero caller-method calls;
the typed-member proposal still refuses for missing original-input identity.
The consumer has zero TypeScript diagnostics. This pins the distinction between
the new whole-root premise and a later member claim without relying on the
producer's nested-callable exclusion.
