# Restricted type-erasure boundaries

ADRs 0026, 0028 and 0030 admit `node-strip-inert-esm-v1`,
`node-strip-import-free-esm-v1`, and
`node-strip-relative-ts-graph-esm-v1` as controlled execution consumers. The adjacent
`probe-source-disposition` tracer uses a typed identity function to exercise
the import-free profile's native census → exact erasure → derived veto →
receipt → recipe-replay chain.
`probe-recipes/derived-veto.mjs` deliberately emits the contradiction marker
after loading and checking the stripped function. It proves that a derived
gate contradiction still refuses closure; it is not a real-package defect.
The source/output/Node-pin and scoped receipt mutation checks live in that
tracer. Imports and transform-requiring syntax are outside the import-free
whitelist, owned by `solid-facts::ast::import_free_erasure` and tested there.
The inert profile continues to refuse executable expressions and reflection.
Receipt mutations are canonically encoded and re-signed with the valid test
issuer, so their refusal tests the consumer's live binding rather than merely
a broken signature. The tracer also tries ordinary issuance with the complete
profiled evidence and requires `ControlledExecutionRequired`.
`inert-import.ts` and `inert-unsupported.ts` are the named profile refusals for
unresolved imports and enum transformation. `reflect.ts` is admitted only as
Node-strip behavior; its receipt cannot be reused by the TypeScript compiler
whose observably different output is pinned below.

ADR 0028 resolves ADR 0025 only for a checker-owned immediate execution of one
exact import-free derived module. It does not establish that a project executes
the same bytes, so no application compiler or bundler can consume its receipt.

`probe_type_erasure_reflection_does_not_establish_consumer_compatibility`
launches the build-pinned Node against `compare.mjs`. It derives both outputs
from `reflect.ts` in memory: Node strip-only and the locally installed,
lock-pinned TypeScript 5.9.3 ESNext compiler. The typed source checks reflected
function spelling. The first execution observes no global addition; the second
does. A third, independent source confirms that an addition in stripped code
is observable. Enum and TSX inputs requiring more than stripping refuse.
The global addition is a contradiction tripwire, not an exhaustive observation
of the package-contract creates domain. No receipt or native census is issued
by this experiment. The global's declaration keeps the source type-correct.
`extensionless.mjs` cannot load `./dependency` despite a published
`dependency.ts` sibling. The comparison requires Node's `ERR_MODULE_NOT_FOUND`,
pinning that authenticated file availability does not establish resolution.

`probe_execution_profiles_refuse_at_the_existing_consumer_boundary` exercises
the actual native receipt loader, with a valid receipt as its positive control.
An added profile field at the receipt payload refuses before authentication,
even for `null`, a published-byte label, or a well-shaped erasure description.
Profile additions to `Policy2ReceiptBindings` and `ResolvedImport` likewise
refuse; a source or transform digest is not a compatibility capability.

The adjacent fixture keeps the ordinary TS-only `IncompleteGate`, completes
that candidate through the explicit import-free profile, and certifies the
published-JS sibling normally. Production controls cover source, derived,
retained-output, transformer, profile and receipt mismatch, plus a contradiction
in the derived bytes. Existing write and timeout controls remain authoritative
for the shared harness. `relative-root.ts` and `dependency.ts` pin the positive
extensionless-import graph. The production tracer checks exact native-replayed
edges, source/output/profile mutations, an unmapped import, ordinary-consumer
refusal, and `relative-derived-veto.mjs`'s derived-byte contradiction. Browser
execution remains explicitly withheld by ADR 0031; ADR 0032 reserves the
profile name `chromium-headless-shell-cdp-pipe-esm-v1`, and the same tracer
pins that it is refused as an unknown controlled profile.

This directory has no package manifest or generated main document. It is read
by native regression tests and does not increase phase19's stable-main count.
