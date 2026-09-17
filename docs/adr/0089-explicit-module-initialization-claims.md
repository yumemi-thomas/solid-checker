# Explicit module initialization claims

Status: explicit claim, generation, planning and ordinary receipt loading
implemented; six scoped entrypoint occurrences newly certified, with all nine
prior cases preserved in combined publications. Full-corpus measurement remains.

The public document and normalized artifact case now represent
`initialization: "inert"`. Missing means no claim; explicit null and unsupported
values refuse decoding. A separate canonical digest domain binds this additive
meaning while documents without the claim retain their old digest stream.
Certification planning independently parses authenticated runtime bytes and
retains the opaque proof in the plan and module-closure witness. Transformed
cases refuse. `.js` requires the nearest package scope in the complete
authenticated archive to declare `type: module`; nested missing-type,
CommonJS and malformed manifests cannot inherit the root's permission.
`.mjs` has an explicit module-format premise. Focused tests cover these
boundaries and document round-trip/identity mutation; both pass.

The model/planning slice passes full `make verify`: actual exit 0,
`TOTAL 140.61`, no `FAILED during step` marker, log
`/private/tmp/inert-initialization-model-verify.log`, with
`GOCACHE=/private/tmp/solid-checker-go-cache`. No snapshots were updated.

The end-to-end regression initially refused with `NoClosedClaims`. Policy-2
claim-root derivation now includes an explicit, domain-separated inert-module
claim bound to the normalized semantic digest and exact case id. An absent
claim and empty export census still refuse. The test issues a receipt only
after snapshot planning proves initialization, loads it through the ordinary
consumer, and rejects a changed importer, removed claim and effectful source.
The generation seam additionally rejects a proof for different runtime bytes.
All three focused tests pass through the pinned `make test-focused` target.

Final acceptance validation passes full `make verify`: actual exit 0,
`TOTAL 131.55`, no failed-step marker, log
`/private/tmp/inert-initialization-acceptance-verify-fixed.log`. The first run
exposed accidental proposal generation for empty TypeScript; generation now
requires untransformed `.js`/`.mjs`. The only updated fixture is
`wildcard-asset-entrypoints`: its empty/comments-only JavaScript cases now
explicitly propose inert initialization, while asset dispositions are unchanged.
All 96 contract-corpus fixtures pass. The combined package probes were repeated
with the corrected fresh binary and their final catalog identities are recorded
in the measurement. No unrelated snapshots were updated.

The [scoped measurement](../package-contract-v2/phase21/2026-09-09-inert-initialization-scoped-measurement.json)
follows published case-set pointers, catalog references and their digests.
All six new `./client-only` and `./server-only` occurrences authenticate and
select through ordinary consumers. Their separate catalogs leave the previous
three cases per row intact: `./plugin/rsbuild`, `./plugin/vite`, `./server-entry`.
The follow-up combined transactions publish five cases for each row. All nine
prior canonical main documents are byte-identical, and each combined set passes
ordinary consumer authentication and exact selection. The latest full corpus
remains 327 complete, 64 partial, 18 refused and 9 not advanced,
with 1,535 cases; do not relabel those old artifacts as a new full run. The
three target rows remain partial: their roots and other explicit entrypoints
are still missing from this scoped work. No denominator correction was made.

The parser-level premise is implemented in
`rust/crates/solid-facts/src/ast/inert_javascript.rs`. It recognizes only
comments/whitespace, empty statements and local `export {}` under the actual
JavaScript module grammar. Its non-serializable result privately binds the
source SHA-256, byte length and complete statement count; a caller cannot
deserialize or construct a result from an empty export list. Source size is
bounded before parsing. Parser panic/errors, directives, imports (including
`export {} from ...`), executable declarations, calls, writes, JSX and ambient
TypeScript all remain refusals. It does not change the existing emission
classification or certify any package by itself.

Both focused tests pass, covering exact-byte binding, Unicode comments,
effectful and malformed controls, and the source limit. Full `make verify`
passes with actual exit 0, `TOTAL 145.26`, and no failed-step marker in
`/private/tmp/inert-javascript-facts-verify-cache.log`. Earlier attempts stopped
on formatting and the sandbox-inaccessible default Go cache; the passing run
uses `GOCACHE=/private/tmp/solid-checker-go-cache`. No unrelated snapshots were
changed. That earlier run covers the parser slice only; the model/planning
validation above is newer. Generation and end-to-end receipt/consumer tests
subsequently completed as recorded above. The scoped package measurements are
separate from a full corpus rerun.

ADR 0088's CommonJS prototype cannot become an ordinary certificate merely by
authenticating its host snapshot. ADR 0025 and `controlled_execution.rs`
already establish this boundary: controlled-execution receipts deliberately
fail ordinary policy-2 authentication. A host session must not be hidden in
`producerSessionsRoot`, `transformRoot` or an issuer signature while ordinary
applicability remains unchanged. Shared-interface ownership is resolved; the
missing applicability mechanism remains implementation work, not a permission
block.

An independent prerequisite is representing module initialization itself.
The current contract model describes exported values and call domains; it
has no explicit claim for evaluating an exportless module. An empty export
map cannot stand in for such a claim. This affects both CommonJS initialization
and ESM side-effect/marker entrypoints.

## First claim and proof boundary

Add an explicit, artifact-case-scoped initialization claim for a positively
proved module with no evaluation work. A verifier must independently parse
the exact authenticated runtime bytes as executable JavaScript and establish
the complete statement list and import/export surface. Unknown parse results,
empty producer responses or a missing source file cannot establish it.
Declaration-only TypeScript must not be interpreted as executable JavaScript.
The existing exact manifest, importer, conditions, declaration closure, trust
and artifact-selection checks still apply.

This claim is about the selected published module's evaluation only. It does
not establish that an application builds, that framework plugins preserve
these bytes, that an intended API was shipped correctly, or that a worker
handler/registration module has no effects because it exports nothing.
Side-effect imports, calls, property writes, initializers and unsupported
syntax must retain an explicit unproved initialization obligation.

The claim must appear in normalized applicability and in the canonical public
document, contribute to semantic identity and proof demands, and be verified
by ordinary receipt loading. It must not be a generator-only flag. Old
consumers must refuse the new meaning rather than silently accept an empty
export map. This requires coordinated schema, normalization, generation,
proof, publication and consumer tests before any benchmark gain can be claimed.

## Retained target evidence

The latest full report remains
`rust/target/ecosystem-investigations/2026-09-08-original-helper-full.json`,
SHA-256 `7ee58c116e8acff5b96359be7e7be1372aabab56db93c59bc82f198756c6aea2`.
Its retained projects expose two exact targets for each of these three rows:

- `@tanstack/solid-start@1.168.47|solid1|only`;
- `@tanstack/solid-start@2.0.0-rc.2|solid2|floor`;
- `@tanstack/solid-start@2.0.0-rc.2|solid2|head`.

`./client-only` selects `./dist/esm/client-only.js` and
`./dist/esm/client-only.d.ts` under the explicit `import` branch;
`./server-only` selects the corresponding `server-only` files. Each runtime
file is zero bytes, SHA-256
`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
Each declaration file contains `export {};` followed by a newline, SHA-256
`8e609bb71c20b858c77f0e9f90bb1319db8477b13f9f965f1a1e18524bf50881`.
The manifests state `type: module`. These are discovery facts to authenticate
again inside certification, not receipts.

The possible initial gain is six entrypoint occurrences across three rows,
with **zero complete-row transitions** because other missing entrypoints and
the declared package.json denominator remain. The scoped transactions now
establish all six additions; none establishes a complete row.
The subsequent [Solid Devtools scoped measurement](../package-contract-v2/phase21/2026-09-09-inert-devtools-scoped-measurement.json)
certifies the `solid-devtools@0.34.5` root under `import`, selecting its
zero-byte `dist/index_noop.js`. Its declaration imports `./setup`; the
certification transaction verifies that declaration closure. The published
two-case set preserves the prior `./vite` main document byte-for-byte, and
ordinary consumer verification authenticates the receipts and exact cases.
This is one further certified case, outside the six SolidStart additions,
with no complete-row transition. The development root, `./setup`, `./babel`,
and package.json coverage distinctions remain. The pinned release transaction
reports about 1.6 seconds across acquisition, generation and certification
stages; no source change or additional full-suite run was needed.
Diagnostics' Vitest entrypoint executes
`expect.extend`, and SSE's worker handler installs listeners; neither belongs
to this first claim.

The existing `ModuleEmission::Empty` and `NonDeclaring` distinctions and their
deliberate refusals remain unchanged. They are evidence for discovery, not
permission to clear the refusal. Recovering these targets requires the
explicit positive claim above, with focused effectful and malformed controls,
published catalog verification, and a preservation audit of prior cases.
