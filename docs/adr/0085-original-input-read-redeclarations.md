# Original-input reads require a unique parameter declaration

The Fractional Indexing helper-read experiment exposed a false premise in
the existing positional read producer. In the following TypeScript-valid
function, `input.slice` reads a local replacement:

```ts
export function check(input: string) {
  var input = "local";
  const first = input.slice(0);
  input = "later";
  return first;
}
```

The positional store census sees the later assignment but not the variable
declarator's initializer. It therefore emitted an original-input read before
that later store, overlooking the earlier replacement. The negative overlay
test reproduced the erroneous fact; TypeScript accepted the snippet with
strict checking and no emit.

Before emitting an initial-parameter-read fact, require that exact parameter
symbol to have one declaration. Multiple declarations need a complete
declarator-store proof before they can establish original value identity.
The guard applies to the witnessed parameter only: a positive control retains
the exact read when a different parameter is redeclared.

This narrows an existing fact to its intended meaning. There is no new wire
field, proof family, receipt interface or dialect behavior, and no new
certification is claimed. The producer source identity and pinned verifier
must be rebuilt together before measurement. The completed retained-
preparation corpus and its matching binaries were archived before this edit;
they remain observations of that earlier build, including this known proof
limitation. The
[overlay evidence](../package-contract-v2/phase21/2026-09-08-fractional-helper-overlay.md)
records the counterexample and the corrected positive and negative controls.

The production initial-read regression suite passes in 1.053 seconds. Full
`make verify` exits 0 with TOTAL 142.98 seconds and no failed-step marker
(`/private/tmp/redeclaration-origin-verify.log`), rebuilding the producer and
matching pinned verifier. No snapshots changed. The fresh full preservation
run is `2026-09-08-unique-read-origin-full.json`, compared with the archived
retained-preparation baseline.

That full 418-row run finished at 22:24:32 JST on September 8 in 1,497.160
seconds with actual exit 0. The unchanged metric remains **327 complete /
63 partial / 19 refused / 9 not advanced**. All **1,534 accepted artifact
cases and their exported claims are preserved**, with zero closure identity
changes, additions or row transitions. Motion Solid 2 floor/head keep `.`,
`./m`, and `./v2`; SolidStart keeps its new `./serialization` case.

The [full measurement](../package-contract-v2/phase21/2026-09-08-unique-read-origin-full-measurement.json),
[all-claim audit](../package-contract-v2/phase21/2026-09-08-unique-read-origin-all-claim-preservation.json),
and [closure audit](../package-contract-v2/phase21/2026-09-08-unique-read-origin-closure-transitions.json)
follow the published catalog pointers and verify exact identities and ordinary
consumer selection. Matching binaries and the producer stamp are archived in
`rust/target/ecosystem-investigations/2026-09-08-unique-read-origin-binaries/`.
This is a soundness correction with measured preservation, not new coverage or
a denominator correction. No snapshots changed, commits were made, or pushes
performed.

The next bounded complete-row candidate remains Fractional Indexing's
helper-read composition for TanStack Solid DB. Its private overlay is positive
for the two original arguments, but no consumer-bound argument-origin field
or helper-read certificate exists. Implementing that shared producer/client
interface awaits the ownership confirmation requested under the user's
coordination constraint. This report does not turn the prototype into proof.
