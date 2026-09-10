# Bounded entrypoint recovery — measured result

Follow-up: the user requested integration into the main worktree. The task-only
implementation and this evidence were applied to
`/Users/thomas/Documents/Github/solid-checker`, preserving its existing changes.
The isolated-worktree references below describe the original verification run;
continued entrypoint work now takes place in the main worktree.

The comparable 418-probe run certifies **320 complete rows**, up from **317**.
Partial rows decrease **51 → 48**; certification refusals remain **30**, and
rows not advanced remain **20**. Exactly the three requested targets move;
every other row's certification/coverage status and every probe's installed
versions match the baseline. This is five new executable entrypoint selections
across three probe rows, not a denominator correction.

| Probe | Before accepted entrypoints | After accepted entrypoints | Transition |
| --- | --- | --- | --- |
| motion-solidjs@0.7.0-beta.4, solid2 floor | `./m` | `.`, `./m`, `./v2` | partial → complete |
| motion-solidjs@0.7.0-beta.4, solid2 head | `./m` | `.`, `./m`, `./v2` | partial → complete |
| @solid-primitives/utils@6.4.1, solid1 only | `./immutable` | `.`, `./immutable` | partial → complete |

The [machine evidence](2026-09-07-entrypoint-recovery-measurement.json) records
every before/after artifact-case selection, published pointer and catalog,
runtime/declaration/closure hash, exact importer, resolution trace, receipt
payload, dependency receipt/trust root, and measured binary identity. It follows
only published pointers and named catalogs, checks their content digests, and
filters exact package name/version. It is diagnostic evidence, not authority.
The native transaction authenticates receipts and independently reconstructs
the expected case coordinates through ordinary consumer discovery; all three
targets report both `receiptAuthenticated: true` and `exactCaseSelected: true`.

## Artifact selections and consumer evidence

Both new motion subpaths select `./dist/v2/index.mjs`, SHA-256
`df074aed506a5b4f7d97e79e7108eac89f9a1f682eaa8648b44b296338292e9d`, with
`./dist/v2/index.d.mts`, SHA-256
`01fc006a4933e6b084b152285f59a41017f7c5ffe6249f0d94d64bac5735711e`.
`.` and `./v2` remain distinct entrypoint/branch selections despite sharing
file bytes; floor and head retain their separate Solid resolution contexts.

The new accepted motion cases bind `MotionGlobalConfig` to the exact runtime
declaration in `motion-utils/dist/es/global-config.mjs`, digest
`692ecbabc8f05f1e4ad9f293cc038897259c3f848f814ba1b6978ed7cd0b1818`, and its
declaration in `motion-utils/dist/index.d.ts`, digest
`ffeb91f2c925ded5147acdeaef2b5060a8e48f5c5e528e646ea872f4440bd2be`.
The accepted summary is `{ "call": {}, "shape": "plain" }`. Existing Solid 1
support was a lead for investigation; no Solid 1 receipt was transplanted.

The preserved motion `./m` selects `./dist/v2/m.mjs`, digest
`6a4b2ab445bc9aae161d6dba35c090b518d0533c5f794e5c6d37342b3cd24a8a`, and
`./dist/v2/m.d.mts`, digest
`ce1092603e825395617502553983efb2ccbb2149ec53ba85523f5ab631784d7f`.
The utils root selects `./dist/index.js`, digest
`e899c544004dc1eea92cdfbd35b073e02f57eed58f300a1c8b3e70e6668d974b`;
its preserved `./immutable` selects `./dist/immutable/index.js`, digest
`603f6152a0b62abc4de854caaf6e2d54eacff2bbc37df2eea0a1603001627b3d`.
Exact declaration selections for utils are included in the machine evidence.

All previous entrypoint selections preserve runtime bytes, declaration bytes,
and export branches. Their receipt, importer and closure identities are freshly
bound; the graph closure includes authenticated dependency contracts. Preservation
does not mean byte-for-byte reuse of the old receipt or old artifact-case ID.

## Implementation and bounds

`packages/cli/scripts/certify-contract.mjs` adds opt-in `--recover-entrypoints`.
It prepares the generated cases alongside the exact dependency-composition
frontier, including when generation inputs are reusable. The existing native
graph case-set transaction re-certifies every root, composes dependencies in
its own context, rejects incompatible selections, and verifies publication.
The ordinary graph-only flag retains its existing behavior.

The new coordinate selector rejects duplicate selections, including equivalent
spellings of the implicit `import` condition. It preserves other explicit
refusals under `graphPreparation.entrypointRecovery.remainingRefusals`; it does
not add an absent or unattempted case. Preparation failure retains the partial
proposal with an explicit trace. Native proof failure refuses publication.
Existing native node/depth limits and publication checks remain in force.

The benchmark can enable recovery for exact IDs using repeated `--recover-probe`
without filtering the rest of the corpus. The final run enables only the three
rows above. All three audits state `reusedProposalForRecovery: true`, proving
that the reusable-proposal early return no longer prevents this opt-in path.
No protocol, receipt, Rust certification interface, proof rule, trust policy,
TypeScript diagnostic boundary, or coverage metric changes.

## Measurement and validation

Baseline: `rust/target/ecosystem-investigations/2026-09-07-union-full.json`,
finished **2026-09-07 16:06:25 JST**, newer than the requested 15:36 report.
The baseline has 418 probes, 307 package versions and 214 package names.

Final full report:
`/private/tmp/solid-checker-motion-evidence/recovered-full-with-recipes.json`.
It uses `scripts/ecosystem-benchmark/probe-recipes`, matching the normal
ecosystem Make targets, and an immutable copy of the pinned verifier built by
`make verify`. The evidence records SHA-256 identities. The separate debug
build used `make build-checker-debug` and a separate target directory.

- Focused CLI suite: **176 tests passed**, plus the CLI TypeScript check.
- Benchmark routing suite: **63 tests passed**, including targeted recovery,
  an unrelated probe retaining its normal lane, proposal preservation, and
  graph-only routing. Workflow tests cover duplicate conditions, retained
  cases, explicit remaining refusals, and invalid/missing coordinates.
- Fresh scoped motion and utils probes: certified with all expected cases.
- Final comparable full ecosystem run: **418 probes, exit 0**, precisely
  the three transitions above; no coverage regressions.
- Full `make verify`: **exit 0**, `TOTAL 193.50s`, no `FAILED during step`
  marker. Earlier full verification also passed in 192.18s. Armed process
  tests, coverage, ownership, contract corpus, TypeScript oracle, formatting,
  Clippy, schema and conformance gates ran through the Make workflow.
- Task-only patch: `git apply --check` passes against the shared checkout.

An initial test invocation incorrectly appended a file argument to `tsc -p`;
the corrected complete CLI command passed. A preliminary full run omitted the
recipe corpus and is excluded from the comparable result: it changed withheld
claims and one unrelated row. One follow-up with an incorrect recipe path was
interrupted before producing a report. Neither is credited as a gain.

The initial offline graph attempt encountered missing exact archives for
`@solid-primitives/refs@3.0.0-next.0` and
`@solid-primitives/transition-group@2.0.0-next.0`. After explicit user
authorization, normal acquisition verified their lock-pinned archive integrity.
Installed directories and receipts from other contexts were never substitutes.

## Remaining shortfall and handoff

Both motion rows achieve the target ceiling of two complete-row transitions;
utils adds the measured secondary transition. No missing executable entrypoints
remain in these three rows. Complete entrypoint coverage is not complete
behavioral knowledge: claim-local unknowns and withholdings remain.

The full run has **1,043 withheld closure-candidate occurrences**, versus 462
in the baseline. Additional certified dependency graphs enlarge this census;
this task does not claim creates-refusal reduction. The 48 remaining partial
rows retain the existing denominator distinctions: 25 asset/type-only gaps,
8 wildcard rows, 3 deliberately scoped probes, and 12 executable-gap rows.
The 30 refused and 20 not-advanced rows are unchanged. No unmeasured future
gains or denominator corrections are counted.

Implementation and evidence were subsequently applied to the main worktree at
the user's request, preserving its ongoing changes. The isolated worktree and
patch are historical evidence only. No snapshots or public contracts were
rewritten, and nothing was committed or pushed.

## Main-worktree continuation

Full `make verify` in the main worktree passed with **exit 0, TOTAL 78.36s**
and no `FAILED during step` marker. The log is
`/private/tmp/entrypoint-main-verify.log`. The 176 CLI tests and TypeScript
check, 63 routing tests, and 27 report tests also passed. An initial sandboxed
verification could not access Go's cache; the authorized rerun passed.
Reports now preserve the recipe-corpus path (including explicit absence) and
the requested recovery selection, so future comparisons retain these inputs.
This does not change the coverage metric.

A further scoped measurement is retained at
`rust/target/ecosystem-investigations/2026-09-07-sse-recovery.json`.
It used the same pinned immutable verifier and checked-in recipe corpus.
All three SSE probes gained a certified root while preserving `./worker`:

| Probe | Before | After | Row transition |
| --- | --- | --- | --- |
| `@solid-primitives/sse@0.0.103`, Solid 1 | `./worker` | `.`, `./worker` | partial → partial |
| `@solid-primitives/sse@1.0.0-next.2`, Solid 2 floor | `./worker` | `.`, `./worker` | partial → partial |
| `@solid-primitives/sse@1.0.0-next.2`, Solid 2 head | `./worker` | `.`, `./worker` | partial → partial |

The exact before/after artifact-case tuples, published pointer and catalog
digests, importer identities, receipt payloads and dependency bindings are in
[the SSE evidence](2026-09-07-sse-recovery-measurement.json). Each native audit
reports `receiptAuthenticated: true` and `exactCaseSelected: true`; retained
runtime and declaration hashes and export branches match the earlier cases.
These are three additional entrypoint certifications, **zero additional
complete rows**. This is a three-probe measurement, not another full corpus
run; it must not be substituted for the comparable 418-row totals above.

Every SSE row still refuses `./worker-handler`: its published JavaScript
registers worker message handlers but has no runtime ESM exports. This is an
executable side-effect module, not an asset/type denominator correction.
Supporting such an entrypoint requires a positive module-level proof model.

Two further retained-project graph attempts exposed bounded next blockers:

- `@solidjs/start@2.0.3`: graph preparation cannot resolve the virtual
  `solid-start:app` imported by `dist/client/StartClient.jsx`. The existing ten
  entrypoints remain accepted; no new case is credited. Audit output is under
  `rust/target/ecosystem-investigations/frontier-C4Pslj`.
- `@tanstack/solid-table@9.1.2`: eight requested cases prepare, but native
  certification refuses `./static-functions:cell_getIsAggregated` because
  signature-census alternative 0 lacks `column.table.atoms.grouping.get`.
  Demand: `03d3d72159688acd7359e3e36c35ffbe3f6b65fe11f61c0193606821978d41b9`.
  The separate original `./flex-render` publication remains untouched; the
  attempted combined set is not published. Audit output is under
  `rust/target/ecosystem-investigations/frontier-cUGb0C`.

These attempts used retained exact artifacts with network acquisition disabled.
No proof interfaces, trust rules, virtual-module substitutes, or creates
premises were changed to bypass these blockers. Further complete-row gains
remain unmeasured.
