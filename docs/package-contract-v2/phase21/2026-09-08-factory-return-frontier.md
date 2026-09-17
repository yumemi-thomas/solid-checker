# Factory return frontier: investigation only

The retained Solid 1 graph refuses `seroval-plugins@1.5.6`'s
`AbortSignalPlugin`: its declaration is `any`, which supplies no positive
non-callable/non-constructable export-root premise. Its implementation calls
`seroval@1.5.6`'s `createPlugin`. Reading the published identity implementation
does not authorize composition without an accepted contract and exact binding.

Fresh generation from the retained Solid 1 installation produces no return
identity for `createPlugin`; it proposes only callable shape and empty creates.
This is an unaccepted proposal, not new certification. Evidence is retained at
`/private/tmp/seroval-return-evidence-n1EIDe/inspection.json`. Its production
runtime hash is
`a90cdef339b0f821a7af1e2fefa60a6165dde5090b45aab0cf78cf1643c52e6a`.

A separate minimal packed-package diagnostic in
`/private/tmp/return-alias-diagnostic/result.json` demonstrates that generation
of `function ai(e) { return e; } export { ai as createPlugin };` omits return
identity alone but emits it when an unrelated object literal is added. The IR
structured-return discovery seed excludes ordinary parameter returns. This
diagnoses a generation inconsistency; it does not establish that changing the
seed recovers the Seroval contract or the blocked export root.

Before implementing factory composition, establish an accepted exact return
identity, bind the export initializer and argument allocation, and bind the
dependency receipt in the consumer. A call-result transcript cannot substitute
for an export-value transcript. Missing types confer no permission. The
separate retained `createResource` tuple-path blocker also remains. No proof
rule, receipt interface or certification claim was changed in this investigation.

## Context isolation result

Further generation diagnostics narrow the actual Seroval omission to Solid 1
unresolved-claim attribution, rather than the structured-return seed:

- The full production bundle, original declarations and `.mjs` extension emit
  parameter-return identity in an isolated package under the default dialect.
- Moving that diagnostic package under `node_modules` preserves the claim.
- Adding the retained Solid 1 package manifest above the identical diagnostic
  changes dialect selection and removes the return claim.
- Observing the native emitter's existing stderr records shows
  `ReactiveDispatchUnresolved` obligations for member calls elsewhere in the
  bundle, attributed by `fallback-all` to all exports including `createPlugin`.
  The records erase `reactiveReads` and `returns`.

The diagnostic outputs are under `/private/tmp/seroval-isolated-export-diagnostic`,
`/private/tmp/seroval-original-types-diagnostic`,
`/private/tmp/seroval-full-export-diagnostic`,
`/private/tmp/seroval-mjs-export-diagnostic`, and
`/private/tmp/seroval-node-modules-diagnostic`. The last directory contains the
Solid 1 result. Native attribution records are retained in
`/private/tmp/seroval-native-observer.log`. All runs exited 0, used the pinned
verify binary and producer, and acquired no network artifacts. These copied
packages are diagnostic artifacts, not substitutes for published package cases.

The next bounded implementation should preserve an independently established
parameter identity only with a positive premise that makes unresolved dispatch
irrelevant to that return. It must not interpret missing reachability as proof
that an obligation cannot affect an export. Native return-identity verification
and the still-missing export-initializer composition premise remain necessary.
No new executable case is certified by these diagnostics.

## Prerequisite after ADR 0074

ADR 0074 recovers an accepted parameter-return operation for Seroval in a fresh
ordinary transaction. Its `returns` domain remains open. That is not an
exhaustive identity-factory contract: another return operation is still allowed
by the accepted semantic document, regardless of the stronger facts seen during
verification. Factory composition must consume the committed meaning, not
unpublished knowledge from that verification session.

The current `census_returns_domain` explicitly refuses every nonempty returns
enumeration (ADR 0035). The next prerequisite is therefore a bounded nonempty
closure rule: a single whole-parameter return operation, plain completion form,
and a complete accounting of value-carrying returns positively bound to that
same original parameter. Unknown or unclassified flow, another returned value,
async/generator completion and missing parameter identity must refuse. The
existing `require_returned_parameter_identity` supplies part of that proof but
must not by itself be mistaken for an accepted closed-domain claim.

Separately, the retained Solid 1 proposal's `./dist/dev.js` and `./dist/solid.js`
cases select their own JavaScript as both runtime and declaration. They propose
a `createResource` tuple with an accessor at index 0, while the signature census
does not establish that tuple path. This is distinct from the factory blocker;
neither a borrowed root `.d.ts` binding nor absent tuple facts can discharge it.

The post-ADR-0074 retained Solid 1 recovery completed in 183.581 seconds with
exit 0, no archive/metadata cache misses, and ordinary receipt authentication
and exact case selection. It still falls back from the graph lane. The
[published-set comparison](2026-09-08-dispatch-identity-solid1-measurement.json)
against the latest retained full corpus records 29 → 29 artifact cases and
16 → 16 entrypoint names, with zero lost selections and zero changed claims.
The row remains partial. Thus the measured entrypoint gain of ADR 0074 on this
target is zero; the newly accepted Seroval premise is not counted as a corpus
transition. Result: `/private/tmp/retained-graph-native-azmpxD/result.json`;
log: `/private/tmp/dispatch-identity-solid1-recovery.log`.
