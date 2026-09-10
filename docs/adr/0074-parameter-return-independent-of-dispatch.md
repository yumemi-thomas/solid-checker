# Preserve parameter identity under unresolved dispatch

Solid 1 generation of Seroval 1.5.6 erased `createPlugin`'s inferred parameter
identity because unrelated unresolved member dispatches were attributed to all
exports. That broad attribution must remain conservative for reactive reads
and structured return descriptions: an omitted member could be reactive.

A positive `argument` return with a parameter index and no structured children
describes identity only. It makes no negative claim about members or the value's
reactivity. Preserve that proposal under `ReactiveDispatchUnresolved`, while
still opening reactive reads. Other obligations retain their existing domain
erasures. In particular, unknown or absent return claims do not qualify, nor do
tuples, objects, accessors or callback-result relations.

This changes proposal attribution in the native CLI, not certification or trust.
The return operation remains partial with zero lower bound. Native certification
must independently bind the original parameter and every return site. The
attribution log separately reports the actual domains erased for identity
exports, so it does not falsely report that their returns were removed.

Fresh generation against the retained Seroval artifact now emits the identity;
the runtime, declarations and closure selection match the earlier diagnostic.
The result is `/private/tmp/seroval-return-evidence-qMHzVH/inspection.json`.
No executable-entrypoint gain is established yet. Factory export-root composition
and the Solid `createResource` tuple-path blocker remain unresolved. The separate
structured-return discovery seed inconsistency is unchanged.

The ordinary certification lane subsequently published both production and
development Seroval root cases with the parameter-return operation retained.
The [catalog-bound evidence](../package-contract-v2/phase21/2026-09-08-seroval-identity-certification.json)
follows the published case-set pointer, verifies named catalog/document/receipt
digests and records the receipt payloads. Ordinary analysis reports receipt
authentication and exact case selection. This is an accepted dependency premise
in that diagnostic context, not permission to reuse its receipt in another
importer context or a measured full-corpus entrypoint gain.

Validation: pinned debug build passed; the binary regression passed (one test,
60 filtered); retained Seroval generation passed; ordinary certification exited
0 in 7.317 seconds with no cache misses or network acquisition. Full `make
verify` exited 0 with TOTAL 84.23 seconds and no failed-step marker in
`/private/tmp/dispatch-identity-verify-formatted.log`. The first verification
attempt stopped at formatting, corrected before this successful run. The
library-only focused helper selected no tests; the reported regression result
comes from the subsequent pinned binary-test invocation. No snapshots or bundled
contracts were changed by this slice. Full-corpus coverage has not been rerun.
