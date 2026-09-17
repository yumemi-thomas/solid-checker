# Exact declaration-subject recovery

Baseline: `2026-09-07-unwritten-parameter-bound-full.json`, finished at
2026-09-07 18:49:42 JST: 418 probes, 321 complete, 51 partial, 26 refused,
20 not advanced. No newer full report existed at this investigation's start.

## Bounded opportunities

Visibility Observer 2.0.1 refused its root because the private Type Facts
program resolved `createPageVisibility` in `dist/index.d.ts` while the exact
Node case selected `dist/server.js`. The manifest explicitly maps `node.import`
to that runtime, and no matching server declaration sibling exists. The
existing source-declaration lane selects server.js; importing the public package
under default bundler conditions selects a different module. Singleton custom
condition cases now use the same exact snapshot declaration harness already
used for multi-case batches. Node/browser/default regression tests distinguish
those paths. The scoped report `2026-09-07-conditional-harness-visibility.json`
certifies the root and passes ordinary receipt authentication and exact case
selection: refused to complete, without changing the denominator.

Solid DB 0.2.40 refused its namespace re-export because verification prefixed
the declaration path `dist/esm/query/ir.d.ts` with the parent package instead of
its authenticated owner `@tanstack/db`. The dependency fallback compared the
compiler's quoted module identity with the export spelling `*`. A narrow
namespace path now requires the replayed owning snapshot, its explicit namespace
target, the corresponding private-project path, and an exact compiler module
identity. Published test archives verify positive ownership and refusal for
missing dependencies, changed snapshot bytes, wrong names and foreign paths.
The scoped report `2026-09-07-namespace-owner-db.json` certifies the root through
the published graph and passes ordinary receipt authentication and exact case
selection. The row moves refused to partial (1/2 declared entrypoints); sixteen
closure candidates remain explicitly withheld. No closure refusal was removed
to obtain the entrypoint certification.

[ADR 0057](../../adr/0057-exact-conditional-and-namespace-subjects.md) specifies
the premises. No Type Facts protocol, receipt format, trust rule or external
behavior model changes. No creates-refusal work is included.

## Other inspected blocker

Testing Library reaches `dom-accessibility-api@0.5.16`, whose import branch
selects `dist/index.mjs` without a matching `.d.mts`. The existing resolver test
explicitly rejects borrowing cross-format `.d.ts`/`.d.cts` declarations. This
investigation does not change that rule or claim a certification for the row.

## Validation

The focused native tests `a_single_conditional_case_uses_its_exact_declaration`
and `dependency_namespace_subject_requires_its_replayed_owner` passed. Both
package retries used a fresh `make build-checker-debug` binary and armed
`SOLID_TYPEFACTS_BIN`; both ordinary consumers reported authenticated receipts
and exact selected cases.

Full `make verify` passed, exit 0, `TOTAL 116.12s`, with no `FAILED during step`
marker (`/private/tmp/exact-subject-verify.log`). This includes 183 CLI tests and
the native/process, coverage, ownership and TypeScript-oracle gates. No findings
snapshots were updated. No commit or push was made. The full-corpus comparison
uses the matching verifier build produced by that pass and unchanged recovery
routing; it must complete before a corpus-wide preservation claim is made.

## Final full measurement

`rust/target/ecosystem-investigations/2026-09-07-exact-subject-full.json`
finished at **2026-09-07 19:30:33 JST**, exit 0, in **644.793 seconds**.
The catalog-bound counts are **322 complete, 52 partial, 24 refused, 20 not
advanced**, compared with **321/51/26/20** at the start of this continuation.
The generation headline in the runner is not this certification metric.

All **765 prior accepted artifact selections across 372 certified rows** remain
present, including multiplicity, runtime/declaration hashes and resolution
branches. Installed package versions match for every probe. Ordinary consumer
verification succeeds for all retained certified rows and both newly certified
rows. There are no coverage regressions, denominator changes or inferred gains.

| Package | Before accepted cases | After accepted root cases | Transition |
| --- | --- | --- | --- |
| `@solid-primitives/visibility-observer@2.0.1` | none | `./dist/index.js` + `./dist/index.d.ts`, branch `/exports/./import`; `./dist/server.js` as runtime/source declaration, branch `/exports/./node/import` | refused → complete |
| `@tanstack/solid-db@0.2.40` | none | `./dist/esm/index.js` + `./dist/esm/index.d.ts`, branches `/exports/./import/default` and `/exports/./import/types` | refused → partial |

These are **three new artifact cases and two entrypoint occurrences**, not a
metric correction. Solid DB's `./package.json` remains in the unchanged
denominator; its root is certified at 1/2 declared entrypoints.

[Exact before/after evidence](2026-09-07-exact-subject-measurement.json) follows
the published case-set pointers and digest-bound catalog references. It retains
every case's hashes, selected branch, importer, signed receipt payload,
dependency receipt/trust roots, checker/producer identities, and the full
preservation census. Each new case was freshly certified; no old receipt was
copied into another context.

Remaining opportunities require different premises: Table's conditional generic
member, SSE module evaluation, Router's graph/runtime obligations, and SolidStart's
application-dependent virtual input remain open. Testing Library's declaration
format refusal was inspected but not changed. This slice adds no new behavior
proof rule and does not continue general creates-refusal reduction.
