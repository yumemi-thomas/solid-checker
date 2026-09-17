# Controlled inert TypeScript execution — 2026-09-05

ADR 0026 was written before implementation. `node-strip-inert-esm-v1` now
completes a real Kobalte source-case transaction through native census,
mandatory derived-byte veto, scoped receipt authentication and fresh controlled
consumption. It admits **one of the original 40 TS candidates: noop in
@kobalte/utils 0.9.2**. It does not grant ordinary analyzer acceptance for TS
sources, and it does not claim that the other 39 can certify.

## Decision and cost

ADR 0025 records the measured alternatives. Keeping refusal preserves the
published-byte meaning but unlocks no source case. Census-only certification
would remove the independent veto and require a different consumer-visible
policy. Published JS siblings exist for all 40 names, but equivalence was
proved for **0/40**; those JS artifact cases can only certify independently.
General pinned erasure with a digest or opt-in cannot establish compatibility
with an application's compiler: the reflection control behaves differently
under Node stripping and TypeScript emission. A controlled consumer can
establish applicability by executing the very interpretation it authenticates.

This first controlled consumer admits one directly exported synchronous
function with no parameters or type parameters and an empty body or bare
return, optionally annotated `void`. The complete Oxc parse decides this
whitelist and computes the only allowed expected erasure. Pinned Node must
independently produce exactly those bytes. This preservation premise is small
enough to review: there is no executable expression, reference, import or
initialization operation that removing the annotation could change. The native
implementation census remains mandatory; a finite clean sample never supplies
positive closure evidence.

The cost is limited applicability. This is a checker-owned zero-argument
invocation, repeated in fresh workers according to the bound schedule. It is
not a contract that a normal Solid application can import. The API returns no
AcceptedContract, function object or replay capability. Receipt version 3 has
an explicit profile/proof identity and separate signature domain, contains no
signed version-2 receipt, and is refused by the ordinary consumer. Each
controlled invocation requires a fresh proof transaction.

The scoped veto still detects contradictions in the derived execution. It no
longer purports to observe an unspecified compiler's interpretation of source
bytes. Erasure can hide contradictions involving reflected function spelling,
source locations or erased module initialization; the broader grammar remains
refused. This profile excludes those constructs, proves the same inert body
with the native census, and enforces its exact derived interpretation at the
only consumer allowed to use the receipt.

## Before and after: same ordinary runner, same corpus

Before is `/private/tmp/erasure-profile-before.json`, taken in the preceding
investigation before these production edits. After is
`/private/tmp/controlled-inert-after.json`. Both use the requested three-row
runner, timeout 600, the fresh Makefile-pinned debug checker, the pinned Type
Facts executable and `scripts/ecosystem-benchmark/probe-recipes`. The checked-in
ecosystem report without a recipe corpus is **not** the baseline. No ecosystem
baseline or phase ledger was updated.

The retained before/after JSON SHA-256 values are respectively
`62897a2787e133b5ea52ab8ba2b2c37bb33a77fdf4495a0368508752de40bb48` and
`38b2b0bfe2fea675de6e28f37b6434f8d355856895256e6abe40c918b5bc8555`.

Earlier work in this dirty tree added six alpha JS candidates, so these fresh
rows contain **49 candidates: 40 TS and 9 JS**. The original population was
43: the same 40 TS and three i18n JS candidates. Keep those denominators apart.

| row | candidates | ordinary before → after | loaded / completed gates | accepted creates closures | exportsProven |
| --- | ---: | --- | ---: | ---: | ---: |
| @kobalte/utils@0.9.2, solid1 | 33 TS | noop IncompleteGate → same refusal; other 32 not run | 0 → 0 | 0 → 0 | 0 → 0 |
| @kobalte/utils@2.0.0-alpha.0, solid2 | 7 TS + 6 JS | two clamp JS gates complete; 11 candidates withheld without recipes, unchanged | 2 → 2 | 2 → 2 | 0 → 0 |
| @solid-primitives/i18n@2.2.1, solid1 | 3 JS | accessor census refuses before any gate, unchanged | 0 → 0 | 0 → 0 | 0 → 0 |

The exact 0.9.2 refusal, reproduced unchanged in the ordinary after run, is:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

The alpha clamp gates complete with no contradiction in both runs:

- import/default: `sha256:115c40ba4502175537c429f74e39b1d8be7cd3e1e189ffd8d8b70f623a654c8d`
- solid: `sha256:008725d9ce46b27c387f0fd3d723aa74ea14ab6dfc212cfb431cf42e2019089b`

Their probe roots deliberately change with worker protocol v3 and sandbox
scheme 8: import/default goes from `9553b0df…537e9` to `2e268142…bdd81`;
solid goes from `2ba8184e…06b3d` to `3c23cd63…d5073`. Their closed claims and
published runtime bytes do not change.

The three i18n gates are all **not run**, because positive census evidence
refuses first:

- scopedTranslator: `sha256:4ce3a2b56edde0cdb56a3fb23c8cd339527a839c3c59931c37fd51f403798d90`
- flatten: `sha256:a40364f9c1352d27cfad1cdc7f7c5d9cb5e4ecaea9820c0c7fad809da3bf1047`
- chainedTranslator: `sha256:d704bf2fcd52710296cf72705e3d9b3ec1eb4ee1e4da88bec6bc3a15bd02f764`

The refusal remains `property-access-unknown-accessor (SpreadAssignment)` at
`dist/index.js:3471..3483`, demand
`sha256:3da8a07cf0cd148afc9153685ee75053ba323a7e207e672f8134e9a9384847f1`.
No accessor-census rule changed in this slice.

## Explicit controlled-profile measurement

`/private/tmp/measure-controlled-inert.mjs` reconstructs version-6 requests
from the retained before-run certification inputs and authenticated cached
archives. It visits the 16 source artifact cases carrying the original 40
candidates. Each case's native planning runs; only the inert case proceeds
through census and execution. Detailed requests, refusals and the scoped
receipt are retained under `/private/tmp/controlled-inert-measurement/`, with
the complete candidate mapping in `results.json`.
Its SHA-256 is
`6ef9212666dac67a014b65a5cbcda53cb6fecbf0b4259efd684c4ec78277cfeb`.

| row | source candidates | loaded | completed gates | real contradictions | controlled accepted closures | profile refusals |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| @kobalte/utils 0.9.2 | 33 | 1 | 1 | 0 | 1 | 32 |
| @kobalte/utils alpha | 7 | 0 | 0 | 0 | 0 | 7 |
| total | 40 | 1 | 1 | 0 | 1 | 39 |

The successful gate is exactly `a9c9b71f…f7bc`, the ordinary row's blocker.
The source and derived hashes both equal
`sha256:a734dcbb74b62cd73a87bc586c5d64562ceb7450e079069b4da7d0dbd44470f4`:
this real noop needs no annotation removal, but still needs the explicit ESM
interpretation under node_modules. The annotated TS-only fixture exercises
actual byte-changing erasure. No package or .d.ts file was changed.

The receipt binds nonempty native DomainExhaustiveness evidence, the exact
source case and output, the compiled Node/harness pins, measured conditions,
empty runtime imports, authenticated dependency materialization and the
controlled interpretation. Its probe root is
`sha256:b657d3827bc87c7eaa33d4f9db5f60d4dfe9175ad0ae65293e8e0a2f6d633bb9`.
The consumer independently reconstructs the module, rechecks pins and output,
then directly calls the export in fresh private workers. The final outcome is
`completed-returned-undefined`.

Thus **1/43 original candidates becomes newly probeable under the
explicit profile**. The original structural probeable population is 3 → 4,
but the three i18n candidates still cannot reach their gates. Ordinary TS
creates-closure acceptance remains zero. `exportsProven` remains zero: a scoped creates closure
does not close the other fact domains of an export.

No real-package contradiction was observed. The derived-veto fixture is a
successful deliberate veto control: it observes erased function spelling and
emits `undeclared-alternative`; native finalization returns
`ProbeGateError::Contradiction` and issues no receipt. The separate reflection
counterexample continues to forbid cross-compiler reuse. These controls are
not counted as real-package contradictions.

## Verification and remaining refusals

All requested checks passed: Makefile debug build and 99 harness tests;
74 facts, 378 backend and 234 IR library tests; armed contracts/diagnostics/
dialects process suites (7/15/37); non-updating contract corpus (94 fixtures);
coverage (94 projects, 547 findings); ownership (289 cases, 465 ledger rows,
zero pending); scripts Vitest suite (154 tests); CLI suite (175 tests plus
types); rustfmt and fmt-check; workspace all-target Clippy with `-D warnings`,
then a separate Makefile pin rebuild. New fixture files were staged before phase19; no
tracked main was added, so its existing stable-main count remains 185.
TypeScript strict/noEmit checks on the new TS fixtures, JSON schema syntax,
dialect manifests and `git diff --check` also pass. The audit caller's protocol
is shared with the worker and checked by a fresh-process regression; Node and
Bun audit smoke runs both complete without granting certification authority.
The phase16 caller was updated to that protocol, but no phase16 report or
ledger was regenerated. Receipt mutation controls use canonical encoding and
valid signatures to test the actual live-binding comparison, and profiled
evidence is explicitly refused at ordinary receipt issuance as well as loading.

**No snapshot moved and no updating gate was run.** Existing dirty-tree
snapshot changes belong to earlier slices and were preserved. No commit,
push, ecosystem baseline refresh, phase-ledger refresh or `make verify` ran.

The 39 remaining source candidates are explicitly outside this initial
grammar. The POC partitions them into 25 further finite-sample completions,
six browser-dependent candidates and eight importing candidates; none of
those figures is positive native certification evidence. The measured eight
importing candidates need three authenticated extensionless edges across six
modules, detailed in ADR 0025. An enforced edge map is still required, and
browser execution still needs a separate profile. The next useful expansion
is pure scalar functions with a reviewed argument representation and erasure
preservation premise, followed by the same census/veto/receipt/consumer chain.
General application consumption, non-inert modules, profile replay and the
independent accessor census remain open.
