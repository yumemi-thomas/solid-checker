# Unwritten parameter identity requires a unique declaration

The user granted ownership of the argument-origin and helper-read
producer/client interfaces. While preparing Fractional Indexing's local
helper composition, a focused negative control exposed an unsound premise
in the existing helper input identity:

```ts
export function check(value: string) {
  var value = "local";
  return value.slice(0);
}
```

The assignment-target census does not classify a variable initializer as an
assignment. `returnedParameterIdentityLocked`, which also supplies unwritten
parameter facts, therefore identified the replacement as the caller's input.
The live producer test failed with one unwritten binding instead of zero.
This is the whole-input counterpart of ADR 0085's positional-read defect.

Require the exact witnessed symbol to have one declaration before granting
original parameter identity. This conservative restriction covers both
unwritten-parameter and returned-parameter facts. It does not invalidate a
parameter because another parameter is redeclared. Supporting multiple
declarations later requires affirmative initializer-store evidence.

Focused regressions cover a redeclaration followed by a member read, a
redeclaration followed by a direct return, and an unrelated redeclaration
that preserves the first parameter's identity. The combined unwritten-input,
initial-read, and helper-binding suites pass in 1.364 seconds. No new protocol
field, accepted case, closure, or coverage metric is introduced by this fix.

TypeScript accepts the exact read, return, and unrelated-redeclaration
controls with `--strict --noEmit --skipLibCheck --target es2023`, exit 0
(`/private/tmp/helper-unwritten-redeclaration-valid.ts`). The private helper
overlay now includes a helper-redeclaration negative control; all 17 cases
pass in 0.329 seconds, retaining the two candidate transfers in the actual
Fractional Indexing 3.4.0 JavaScript. These remain diagnostic candidates,
not new certificates or production helper-composition facts.

The latest completed full corpus remains
`2026-09-08-unique-read-origin-full.json`: 327 complete, 63 partial, 19 refused,
9 not advanced, and 1,534 accepted artifact cases. That measurement predates
this correction; preservation under the corrected build must be measured.
Fractional Indexing's helper composition remains unimplemented. Ownership is resolved;
the remaining work is positive producer evidence, exact client/verifier
binding, focused package certification, and published-catalog verification.

Full `make verify` passed with actual exit 0, TOTAL 142.68 seconds, and no
`FAILED during step` marker (`/private/tmp/helper-unwritten-origin-verify.log`).
No snapshots changed, commits were made, or pushes performed.

The matching fresh TanStack Solid DB graph attempt used the latest report's
retained project, root integrity, and exact dependency-plan root census.
It prepared 19 graph nodes and refused after 8.255 seconds, exit 2, with zero
cache misses. The refusal remains Fractional Indexing's `generateKeyBetween`
original-input proof, demand
`sha256:58f876c6629a299943376a972f5c06af588b813bc4a155390bfe65a2fabdbcb6`,
artifact case
`artifact-case:f610a6ccd396f93016240418d06e43a33bb7531ef8b21055cd363ceea603ab04`.
The complete diagnostic, including binary hashes, is retained at
`/private/tmp/next-explicit-graph-Ocen1V/result.json`.
The first diagnostic invocation could not find an emitted-proposal input
sidecar, because this refused row never emitted one; it performed no
certification. The successful preparation uses affirmative report-bound
root coordinates instead of guessing a sidecar or scanning leftovers.

The row's accepted entrypoint and artifact-case sets remain empty before and
after this attempt; its transition is refused to refused. Corpus-wide
preservation has not been remeasured after this correction. Full verification
is not a substitute for that measurement.

Reconciliation also corrects the earlier target ceiling: this row declares
two explicit entrypoints, root and `./package.json`. The latter is recorded
inapplicable. Under the existing count-based completeness metric, certifying
root alone would move the row from refused to partial, not complete. Thus
the helper opportunity is an executable-entrypoint gain, with zero expected
complete-row gain from root alone. This corrects an unmeasured estimate; it
does not change a denominator or claim a new certification.
