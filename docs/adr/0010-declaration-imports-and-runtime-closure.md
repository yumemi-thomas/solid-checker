# 0010 — Declaration imports do not execute dependency modules

Status: accepted and implemented
Date: 2026-09-04

## Decision, written before code

For a bare external specifier observed while walking an actual declaration file
on the declarations axis, retain the compiler-source acquisition edge but do
not manufacture an all-domain runtime hazard solely because no semantic
dependency receipt was supplied. The generator and snapshot-owned Rust closure
replay must derive the same distinction. Keep any supplied semantic dependency
edge and all export-binding requirements unchanged. Limit this change to
declaration files (`.d.ts`, `.d.mts`, `.d.cts`, and the existing declaration-file
predicate); erased imports in executable TypeScript source are a separate case.

The claim remains defensible because a declaration file cannot execute its
imports. Its authenticated bytes still bind the specifier, the existing
compiler-source channel binds the exact archive/lock/installation coordinates,
and the receipt binds the source set actually used. Every semantic demand that
needs a dependency type must still receive an authoritative Type Facts witness
from that source set. A missing or tampered source cannot become type evidence.
Removing an unrelated runtime hazard does not grant any fact about the
dependency's implementation. Runtime value imports, including empty and
side-effect imports, retain the full authenticated dependency requirement.

## Alternatives and boundaries

Retaining the hazard is sound but prevents a self-contained published JS
bundle from closing any behavioral domain merely because its declarations
refer to JSX. Accepting a semantic contract for the whole JSX runtime would
prove and execute unrelated behavior. Trusting installed declarations or their
types as runtime behavior would be unsound. A new declaration graph format is
unnecessary for this bounded case: authenticated compiler sources and their
receipt root already exist. This decision does not waive an unresolved
declaration export binding, local missing module, opaque asset, or syntax
hazard, and does not add blanket trust for missing types.

The source TypeScript probe refusal in ADR 0009 is unchanged. The `Aliases`
runtime-kind refusal remains Unknown: its separate `.d.ts` cannot prove its
JavaScript initialization. No production harness code or sandbox policy field
changes, so the sandbox scheme remains 6. Node/harness pins, private permissions,
environment, process groups, frame limits, primordial capture/freezing,
resolution echo, and write detection retain their existing requirements.

## Required evidence

Pin a JS package with a declaration-only dependency and its sibling with a
real runtime import; retain the external declaration acquisition census and
show that the runtime sibling still opens/refuses without semantic dependency
evidence. Replay authenticated bytes in Rust and reject a forged omission of
the runtime edge. Exercise the real census and probe on the JS package, plus
existing missing/tampered/wrong-lock compiler-source cases. Re-run the exact
Kobalte JS root measurement and the original three candidate-bearing rows;
report new candidates separately from completed vetoes and closed claims.

## Implementation and measured outcomes

The production change is confined to `closureForRoots` in
`packages/cli/scripts/artifact-resolution.mjs` and `record_external` in
`rust/crates/solid-facts-backend/src/contract_certification/module_closure.rs`.
The external declaration acquisition census is unchanged. Native replay still
rebuilds the closure from authenticated bytes and compares it with the supplied
manifest; the caller cannot mark a runtime import as a declaration import.
No schema or sandbox-policy fields change. The source-evidence root already
binds exact declaration archives, lock selections, installed coordinates, and
the source set actually used by each Type Facts witness.

The fixture pair is `fixtures/package-contracts/declaration-import-closure`.
The native tracer proves the declaration-only package's creates census and
completes its pinned probe, refuses callability with wrong-lock typings, and
rejects a forged omission of its runtime sibling's import. Existing missing,
tampered, substituted-version and colliding-source tests remain green. Added
opaque-specifier tests retain unresolved `#` and native/Wasm boundaries.

All reports below live under the same `probe-ts/` scratch root as ADR 0009.
The baseline is its **recipe-bearing `before.json`**, not the checked-in
ecosystem report. `declaration-after.json` used the original recipe corpus and
isolates the production fix: six new JS candidates appear, but all six are
withheld for missing recipes. The expanded corpus then adds two exact clamp
claims, one per export condition. `declaration-verified.json` and `.md` record
the final three-row run, after rebuilding the pins and with no concurrent Cargo
process. The corpus addition is an explicit second measurement variable.
The final JSON's SHA-256 is
`cc538aa58b7731f172ae99632d33781b40774875c9269ec0c678735f6239978a`.

| row | before | after fix and two clamp recipes |
| --- | --- | --- |
| `@kobalte/utils@0.9.2\|solid1\|only` | 33 source candidates; noop gate incomplete | unchanged: 33 source candidates; same incomplete gate |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | 7 source candidates withheld; no gate | 13 candidates: 7 source + 6 JS; 11 withheld; two clamp gates complete and creates closes in both published JS cases |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | 3 JS candidates; census refuses before gates | unchanged: `chainedTranslator`'s `property-access-unknown-accessor (SpreadAssignment)` at `dist/index.js:3471..3483` |

The six new JS candidates are `clamp`, `getScrollParent`, and
`isPointInPolygon`, each under the import and solid export branches.
**Zero of the original 43 candidates became probeable.** They retain their
exact identities. Instead six independent JS candidates increase the total
from **43 to 49**, and structurally loadable candidates from **3 to 9**.
Four new JS candidates have no recipe, and no claim is made that their census
would pass in this measurement. The subsequent isolated investigation in
`docs/2026-09-04-kobalte-remaining-js-candidates.md` finds that all four refuse
the census before their gates execute. The final corpus addresses six gates
across the rows: two complete,
one is incomplete, and three never execute because the census refuses.
**Zero contradictions; two new certified creates closures.** `exportsProven`
remains **0 on every row** because it measures all-domain export completeness.

Gate outcomes (new IDs derived with `probe_gate_id` from the issued receipt's
snapshot/demand-graph roots and the manifest's exact claim IDs):

- 0.9.2 noop: `sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc`
  — `mandatory probe gate ... did not complete`, unchanged.
- Alpha clamp/import: `sha256:ce06289e646745cd9cf2ba8db28a8989e01ce17a9ada4107ade240067a4b3a7f`
  — completed, no contradiction, creates closed.
- Alpha clamp/solid: `sha256:e066267de9781e7a38839c174e5354e02b76c999149160e9db32e135bf93387f`
  — completed, no contradiction, creates closed.
- i18n `chainedTranslator`, `flatten`, `scopedTranslator`: all unexecuted,
  unchanged from ADR 0009's named gate table.

The two accepted clamp mains explicitly contain `closed: ["creates"]` and
`creates: []`, with nonempty receipt-bound probe roots. The recipes use finite
numeric samples and observe added own global keys during calls. They do not
observe all allocations, initialization before recipe entry, or DOM activity.
As before, the census proves completeness and a probe can only contradict it.
Published JS bytes, resolution, and the original veto guarantees are preserved;
no transformer, source relocation, weaker acceptance mode, or census-only lane
was introduced.

One intermediate expanded-corpus run overlapped a process-test rebuild and
correctly refused 0.9.2 with `probe write isolation was violated: verifier-image
changed while the runtime probe was executing`. It is not the final comparison.
The quiescent rerun restored the exact expected TypeScript load refusal.

## Verification and remaining work

Passed: pinned harness suite **96**; backend `--lib` **362**; IR **233**;
armed process suites **7 contracts / 15 diagnostics / 37 dialects**; a final
focused declaration suite **25** including the added opaque-boundary regression;
CLI **173** plus type tests; scripts Vitest **153**; contract corpus **88**;
coverage **94 projects / 546 findings**; ownership **289 cases / 465 ledger
rows, 0 pending**; schema/dialect validation; fmt followed by fmt-check;
workspace all-target Clippy with `-D warnings`; and diff whitespace checks.
After Clippy, `make build-checker-debug` restored the compiled certification
pins. Cargo steps ran serially through the Makefile. No full `make verify`.

The new fixture directory was added to Git before phase19 ran. It contains no
tracked main document, so `stableMainDocuments` remains **179**. The corpus
matched non-updating: **no snapshot updates**. No benchmark or phase20/21 ledger
was repinned; the lead still owns that work. No commit or push.

The original 40 TypeScript source candidates remain unprobeable. The independent
accessor-census refusals are unchanged. `Aliases` still needs a runtime-kind
proof or a verifier-enforced dependency-export projection; its `.d.ts` alone
does not provide that proof. These are separate next slices, not waived gates.
