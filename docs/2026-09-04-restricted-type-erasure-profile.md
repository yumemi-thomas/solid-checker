# Restricted TypeScript erasure: execution compatibility investigation

Date: 2026-09-04

Production admission remains refused. ADR 0025 was written before the new
regressions and defines the proposed `node-strip-esm-import-free-v1` profile,
its exact meaning and binding requirements. No production loader, derived
receipt issuer, or consumer opt-in was added. The user explicitly authorized
retaining refusal if consumer compatibility could not be established soundly.

The concrete blocker is at the consumer boundary: finalization records an
empty transform schedule, while ordinary contract loading authenticates source
artifact resolution without checking consumer-emitted modules or enforcing
their execution. The POC's reflection control disproves transfer from Node
stripping to an unspecified TypeScript compiler. A hash or opt-in flag cannot
repair that missing premise. A checker-owned runtime capability, or a verified
final-build and execution transcript, must precede production admission.

## Options and recommendation

Keep the current refusal while specifying and testing that boundary. This
retains the published-case claim, census and mandatory veto. Census-only
closure would lose the veto and require another accepted policy; it was not
implemented. Independently certifying published JS remains sound for that JS
case, with 40/40 same-named siblings but 0/40 proved source-case replacements.
Prior work already independently closed five 0.9.2 browser-JS domains and two
alpha clamp domains. Those are preserved.

Restricted erasure is technically feasible for a useful population, but an
admissible certificate must describe derived ESM execution and its consumer
must demonstrably use it. Relocation, extension inference and a transformer
digest alone do not supply compatibility. ADR 0025 enumerates the required
source/output, transformer/Node, format, resolution, dependency and environment
bindings, census correspondence, refusal rules and identity migration. It does
not claim that those production mechanisms are implemented.

The recommendation's cost is explicit: no original source candidate becomes
certifiable in this slice. The benefit is preserving a truthful certificate
meaning instead of transferring a bounded experiment to another execution.

## POC evidence, kept separate from certification

The completed side-conversation POC issued no certification receipts. Its
recorded source inventory comprises the original 40 TS candidates in 16 files.
All parse after pinned Node 24.11.1 stripping. All 32 import-free candidates
loaded; 26 completed finite samples reproducibly; six loaded but required
browser APIs. Eight importing candidates were withheld. Zero real-package
contradictions were observed. All 17 controls passed twice.

Its reflection counterexample is decisive:

> Node strip-only: call enter, call exit.
> TypeScript 5.9.3 emission of the same source: call enter, create-operation,
> call exit.

This is a successful falsification of cross-profile reuse. It is not a real
Kobalte defect and not a native certified creates claim. Type-only import
elision and source/location reflection are relevant even without general
lowering; more general transforms can also change decorator/field timing,
enum reads, helpers, coercion and iteration. A scoped claim is defensible only
for the exact derived execution that its consumer actually uses, with a census
over that meaning and a veto over those bytes. No such compatibility evidence
is available at the present consumer interface.

## Authenticated import investigation

Re-hashed the retained 0.9.2 archive against its original SHA-512 integrity and
read tar members directly; no network, extraction into the repository, package
installation, or execution of the importing candidates was needed.
`/private/tmp/erasure-import-investigation.json` records original and stripped
hashes for all six modules in the three static closures:

| candidates | static edge | exact extensionless file | published candidate |
| --- | --- | --- | --- |
| isVirtualPointerEvent (1) | is-virtual-event.ts → ./platform | absent | platform.ts |
| scrollIntoView, scrollIntoViewport (2) | scroll-into-view.ts → ./get-scroll-parent | absent | get-scroll-parent.ts |
| getAllTabbableIn, hasFocusWithin, isElementVisible, isFocusable, isTabbable (5) | tabbable.ts → ./dom | absent | dom.ts |

Each target has no further static runtime imports. All six strip and parse.
This does not prove absence of dynamic loading, and the `.ts` candidates are
not accepted resolver answers. The native analysis resolver already tries
suffixes; the pinned Node ESM loader does not. A future importing profile needs
an authenticated per-importer edge map, actual loader-result validation,
ambiguity/escape refusal and identical consumer enforcement. Browser execution
is a separate unresolved premise, even after such resolution succeeds.

## Fresh before/after, same recipe corpus

Ran the original command before editing and after verification, with only
`--keep-temp` added. Both runs use the current checked-in recipe corpus, debug
checker built through the Makefile, and `bin/solid-typefacts`. The checked-in
ecosystem baseline was produced without recipes and is **not** this baseline.
No benchmark or phase20/21 ledger was refreshed.

Outputs: `/private/tmp/erasure-profile-{before,after}.{json,md}`.

| report | SHA-256 |
| --- | --- |
| before | 62897a2787e133b5ea52ab8ba2b2c37bb33a77fdf4495a0368508752de40bb48 |
| after | 0be8d9e17860bc578279753def793106bd87725e51165d1ffb57589d59a3f8bc |

Candidate counts are by export and artifact case. Earlier completed slices
added six alpha JS candidates, so the current rows have **49**, comprising the
original 43 plus those six. Of the original 43, 40 are TS and three are JS.
No original candidate becomes newly probeable: **0/43**. Current structurally
loadable JS candidates remain **9/49**; actual loaded candidate gates are two.
POC loading counts must not be substituted for native gate loading counts.

| row | candidates (TS / JS) | loaded gate candidates before → after | completed gates | contradictions | accepted creates closures | exportsProven |
| --- | --- | --- | --- | --- | --- | --- |
| @kobalte/utils@0.9.2\|solid1\|only | 33 / 0 | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 |
| @kobalte/utils@2.0.0-alpha.0\|solid2\|only | 7 / 6 | 2 → 2 | 2 → 2 | 0 → 0 | 2 → 2 | 0 → 0 |
| @solid-primitives/i18n@2.2.1\|solid1\|only | 0 / 3 | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 | 0 → 0 |

Alpha's two accepted closures are clamp on the import and solid JS cases;
the remaining 11 candidates are withheld for missing recipes. The 0.9.2 row
refuses at noop's source load; its other 32 candidates are not executed. Its
failed audit's zero withheld count is not evidence that all candidates ran.
i18n refuses its census before any probe launches. The independent five-domain
0.9.2 browser-JS graph from prior work is outside this three-row comparison;
it is not a replacement for the row's source cases and was not re-counted as
new progress here.

Exact 0.9.2 refusal, identical before and after:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

i18n still refuses demand
`sha256:3da8a07cf0cd148afc9153685ee75053ba323a7e207e672f8134e9a9384847f1`:
`creates census refuses an uncensused invoking form: property-access-unknown-accessor (SpreadAssignment)`
at `dist/index.js:3471..3483`, reach reachable. Only the ephemeral private
directory differs. This independent accessor restriction was not changed.

Every addressed gate, identical before and after (IDs derived with the native
length-framed `probe_gate_id` over the fresh plan/receipt roots and exact corpus
claim; failed-gate identity also matches the emitted refusal):

| gate | SHA-256 ID, prefixed sha256: | outcome |
| --- | --- | --- |
| 0.9.2 source noop | a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc | IncompleteGate; source did not load |
| alpha JS clamp import | 115c40ba4502175537c429f74e39b1d8be7cd3e1e189ffd8d8b70f623a654c8d | completed; closure accepted |
| alpha JS clamp solid | 008725d9ce46b27c387f0fd3d723aa74ea14ab6dfc212cfb431cf42e2019089b | completed; closure accepted |
| i18n scopedTranslator | 4ce3a2b56edde0cdb56a3fb23c8cd339527a839c3c59931c37fd51f403798d90 | not executed; census transaction refuses |
| i18n flatten | a40364f9c1352d27cfad1cdc7f7c5d9cb5e4ecaea9820c0c7fad809da3bf1047 | not executed; census transaction refuses |
| i18n chainedTranslator | d704bf2fcd52710296cf72705e3d9b3ec1eb4ee1e4da88bec6bc3a15bd02f764 | not executed; census transaction refuses |

Alpha's probe roots remain
`sha256:9553b0df05da256f13f4c8808a796b8c12a903dfde4cd8bce1d8d85388d537e9`
(import) and
`sha256:2ba8184e66fb9655b7c9a510620084544a4ca88ce2e5226e0823fba35bd06b3d`
(solid). `/private/tmp/erasure-profile-comparison.json` retains the normalized
comparison; candidate counts, refusals, closure subjects, gate IDs, probe roots
and exportsProven are equal. Accepted closures are distinct from exportsProven:
other claim domains remain open, so exportsProven stays zero.

## Changes and verification

Added ADR 0025, a dated backlog entry, this report and the focused
`fixtures/package-contracts/restricted-type-erasure` controls. Two native tests
exercise the actual receipt consumer's unknown-profile rejection and the
build-pinned Node's reflection counterexample. The latter also checks a visible
derived-code contradiction, enum/TSX refusal and extensionless import failure.
Updated the existing TS/JS fixture README to explain the retained outcome.
No production semantics changed, so sandbox scheme 7, worker protocol,
receipt/proof-policy and public schema identities remain unchanged.

The new directory was staged before phase19. It contains no main document;
stableMainDocuments stays 185. No snapshot update was needed. Existing dirty
changes were preserved, including previously reviewed snapshot moves.

| check | result |
| --- | --- |
| make build-checker-debug; make test-probe-harness | passed; 99 harness tests |
| backend --lib, through Makefile with pins | 378 passed, including the final extensionless control |
| IR --lib | 234 passed |
| armed contracts_process / diagnostics_process / dialects_process | 7 / 15 / 37 passed |
| non-updating contract corpus | 94 fixtures matched; no --update |
| coverage with fresh debug binary | 94 projects, 547 findings matched |
| ownership gate, retained and complete | 289 cases, 465 ledger rows, zero pending |
| Vitest scripts suite | 153 tests, 25 files; phase19 passed |
| bun run --cwd packages/cli test | 175 tests plus type checking passed |
| reflection source against real TypeScript 5.9.3 | strict --noEmit passed |
| cargo fmt --all, then --check | passed |
| workspace/all-targets Clippy -D warnings | passed; separate pinned rebuild followed |
| schema JSON, dialect manifests, diff checks | passed |

Native commands ran one Cargo process at a time through Makefile targets.
Logs are `/private/tmp/erasure-profile-{harness,native,final-pins,scripts,cli,corpus,coverage,ownership}.log`.
No commit, push, baseline/ledger repin or `make verify`; the lead retains that
final integration gate.

## Still unimplemented

Production erasure execution, native source/output verification and census
binding, profile-bearing receipts and consumer applicability enforcement,
authenticated extensionless resolution, and browser execution are all still
open. The POC's mismatch/write/timeout controls are not represented as tests of
an implemented production erasure path. Current harness pin, contradiction,
resolution and isolation regressions continue to test the published-byte path.
No transformed observation was used to accept a source closure.
