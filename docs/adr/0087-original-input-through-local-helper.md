# Original input through one local helper

Protocol 46 adds `originalHelperReads` to an implementation transcript. Each
positive fact binds one original caller parameter, one argument occurrence,
the exact stable local callee, its implementation extent and parameter, and
a direct single-property call on that helper parameter. The caller argument
precedes every possible store to its binding. The helper parameter is
positively unwritten under the corrected unique-declaration predicate.

The producer refuses nested caller callables, module `arguments`/`eval`,
unresolved or multiply declared parameter symbols, writes in sibling default
initializers before transfer, loops that can reorder a store before transfer,
defaulted claimed inputs, overwritten helper bindings, recursive self-hops,
cross-file helpers, spreads before the argument, async/generator helpers,
captured helper reads, and dead calls or reads. Sibling literal defaults do
not establish anything about those siblings' caller values. Discovery is
bounded to 128 caller call records, 32 helper censuses, and 256 facts; exceeding
a bound publishes no helper facts. No external package behavior is assumed.

The client joins each fact to one exact caller signature slot, argument-use
record, resolved call, argument slot and callee declaration. It checks the
same-file implementation extents and rejects duplicate selections. The
declaration's name range and the implementation's full range are separate:
the first live attempt exposed this distinction by refusing a malformed
envelope. The corrected producer states both and retains the exact call's
declaration identity, rather than treating a name range as a function body.

The certifier consumes this fact only for a direct parameter single-member
read whose operation explicitly permits zero executions. It never supplies a
positive lower bound, callable/member shape, returned-value identity, later
input identity, or a dependency receipt. The independent artifact snapshot,
Type Facts source generation, producer identity, proof demand and ordinary
receipt checks remain in force. The public package-contract schema is
unchanged; producer/client handshake and source identities move together.

The focused producer controls exercise before/after stores, sibling defaults,
loops, helper reassignment, captures, dead calls, spreads, parameter and helper
redeclarations, wrong members and slots, and async/generator helpers. Rust
mutation controls reject omitted and duplicate facts, mismatched argument or
helper locations, missing argument uses, captured/dead calls, defaulted inputs,
and a guaranteed-execution request. These are proof controls, not TypeScript
diagnostics or a count of new certifications.

The baseline remains the report-bound 418-row
`2026-09-08-unique-read-origin-full.json` census: 327 complete, 63 partial,
19 refused, 9 not advanced, and 1,534 accepted artifact cases. TanStack Solid
DB has no accepted root case in that baseline. Root recovery alone would be
refused to partial because `./package.json` remains in its declared denominator.
No complete-row or denominator change is implied by this implementation.

The final focused Go helper controls pass in 0.316 seconds. The Rust mutation
test passes through the pinned Make target; the final full workspace run also
includes it. `make verify` passes with actual exit 0, TOTAL 174.49 seconds,
and no failed-step marker (`/private/tmp/original-helper-verify-fixed.log`).
The first full attempt correctly rejected stale schema-digest constants in
both languages; those were updated together before the passing verification.
No unrelated snapshots were rewritten.

A diagnostic native graph transaction completed in 169.642 seconds, exit 0,
with zero cache misses and published TanStack Solid DB's root case. Its
catalog is `/private/tmp/next-explicit-graph-tVf2vo/catalog/accepted-contracts.json`.
This diagnostic preceded the schema-digest correction and is not the final
coverage measurement.

The matching final full corpus completed at 2026-09-08 23:27:54 JST with
actual exit 0 in 1,570.421 seconds. The unchanged 418 probe identities now
contain 327 complete, 64 partial, 18 refused and 9 not advanced rows, with
1,535 accepted artifact cases. The sole transition is
`@tanstack/solid-db@0.2.40|solid1|only`: refused to partial. Its entrypoint set
changes from empty to `{.}`. No complete-row gain or metric correction is
claimed; `./package.json` still prevents completeness under the existing
denominator.

The new root artifact case selects `./dist/esm/index.js`, SHA-256
`7853679165fcf30ebc30694ede9b94b14da0e53b497532b13eb185de0e17d7d1`,
closure `175a9c7c7ee6090ace27eede2e69533dd313da6b0c5f8d96c98e4522ef024977`,
and `./dist/esm/index.d.ts`, SHA-256
`7c94383399b431420239d8fe0739ac77ec67b7c2c188be5cd1e05c75a97a50e4`.
Its runtime and declaration branches are `/exports/./import/default` and
`/exports/./import/types`. Ordinary consumer verification reports both
`receiptAuthenticated: true` and `exactCaseSelected: true`. The receipt digest
is `sha256:b58d5a3f4137defb9c98f70632a8d336d61ddd5ac8d1e1b7ddd35e7c9b67635d`;
its resolved-import root is
`sha256:1cf0d1393696fe5b7a8804cf786f541b11cd0131a0085cdbf83071d093733d0a`.
The catalog binds the exact retained importer and accepted dependency graph;
the receipt separately binds dependency receipts, trust and producer sessions.

The report-bound comparison and all-claim audit are retained in
`docs/package-contract-v2/phase21/2026-09-08-original-helper-full-measurement.json`
and `2026-09-08-original-helper-all-claim-preservation.json`. All 1,534 prior
artifact selections and exported claim sets survive unchanged, including
Motion Solid 2 floor/head `{., ./m, ./v2}`. The corresponding closure-transition
audit records zero identity transitions. These derived audits are not receipt
authority; the report's published catalogs and ordinary verification are.
Matching measurement binaries and their identities are archived under
`rust/target/ecosystem-investigations/2026-09-08-original-helper-binaries/manifest.json`.
