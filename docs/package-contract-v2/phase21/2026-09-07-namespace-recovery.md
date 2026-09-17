# Namespace entrypoint recovery

The final 418-probe run adds one certified executable entrypoint and artifact
selection: **1,488 to 1,489 cases**, preserving every previous selection across
387 certified rows. Counts remain **324 complete / 63 partial / 22 refused /
9 not advanced**. No row transition, metric correction, or further estimated
gain is included.

[ADR 0062](../../adr/0062-namespace-export-binding.md) corrects the exact entity
selected for `export * as Name`, and preserves explicit-export precedence over
an earlier bare star. The source and its declaration facts identify a module
namespace; a same-named callable member is a different export. Existing positive
non-callable/non-constructable evidence now reaches the generator's runtime-kind
reconciliation. Missing entity evidence still refuses. No receipt, trust policy,
Type Facts wire format, or coverage denominator changed.

## Scoped result

The [scoped evidence](2026-09-07-namespace-kobalte-scoped-measurement.json) follows
the published case-set pointer and named catalogs from each retained report row.
Kobalte Core 0.13.13 gains one executable entrypoint and one artifact selection:
576 to 577 accepted cases, 507 to 508 accepted entrypoint names. All 576 old
selections are preserved. The row remains partial because its manifest has
wildcard exports; no complete-row transition is claimed.

The added entrypoint is `./src/index.tsx`. Both runtime and declaration subjects
are `./src/index.tsx`, SHA-256
`d5d7339f1c779e78e9d13c7e3fb8c43322cc0b5f15a0dadd2b6047f4846c1cd8`.
Both resolution branches are `/exports/.~1src~1*`; runtime closure SHA-256 is
`98fb42103a06cf4f25d820d2c49723c15e29343bba17c24340d6f875458779b8`.
The JSON records the complete before/after sets, exact importer and import root,
artifact case ID, catalog/document digests, and receipt payloads. Receipt
identities are scoped to that run, not reusable across its importer contexts.

Native publication reconstructs the expected case coordinates in a fresh
process and uses the ordinary trusted catalog reader to select exactly one
matching importer, specifier, artifact case, semantic digest, and authenticated
receipt. Both staged and committed case sets must pass before publication.
The new namespace case still undergoes archive replay, semantic proof, applicable
dependency and trust checks; correcting a generated proposal alone is not the
reported certification.

## Validation

The normalized-fact test passes and pins the exact namespace binding span.
The new `namespace-reexport-identity` corpus fixture has six successful cases:
namespace, renamed namespace, named callable, bare-star callable, explicit
namespace override, and explicit numeric override. The missing-module case
retains an explicit refusal. Before the precedence patch, both explicit
overrides incorrectly generated callable proposals; TypeScript 5.9.3 accepts
them without diagnostics and gives both exports zero call/construct signatures.
Only this new fixture's three expected artifacts were generated.

Fresh debug coverage passes: 94 fixture projects, 547 findings, no unrelated
snapshot changes. Final `make verify` passes with actual exit 0, `TOTAL 67.59s`,
and no `FAILED during step` marker; log
`/private/tmp/namespace-confirm-verify.log`, exit record
`/private/tmp/namespace-confirm-verify.exit`. The scoped benchmark exits 0 in
403.232 seconds. It precedes the additional explicit-star precedence controls;
the full comparison uses the final verified binary.

## Full comparison

`rust/target/ecosystem-investigations/2026-09-07-namespace-full.json` finished
2026-09-07 at 23:14:01 JST, actual exit 0, 995.531 seconds. Its 418 probes
have no timeout or memory-limit flags. The
[full evidence](2026-09-07-namespace-measurement.json) compares it with
`2026-09-07-retry-sources-full.json`, verifies unchanged installed versions,
and follows the published pointers, named catalogs, main documents and receipts.
The preservation comparison includes each complete artifact selection, including
the runtime closure digest, with duplicate occurrences preserved.

Only Kobalte Core 0.13.13 changes: its old accepted entrypoint set plus
`./src/index.tsx`, and its old 576 artifact selections plus the exact selection
above. Both Motion Solid 2 probes preserve `.`, `./m`, and `./v2` and their
artifact selections. No existing accepted case is lost. Ordinary consumer
authentication and exact case selection pass on all certified rows.

Kobalte still records 41 generation refusals for published test modules with
no runtime ESM exports. Those modules have not been certified or silently
declared inapplicable. Wildcard coverage remains partial. Locator's
browser/development and Router's default-import refusals also remain explicit;
their existing entrypoint-name completeness is not all-conditions completeness.

The overall coverage ceiling remains open. The
[work queue](2026-09-07-coverage-ceiling-plan.md) distinguishes the next exact
program-membership and callback-composition investigations from measured gains.
Nothing was committed or pushed.
