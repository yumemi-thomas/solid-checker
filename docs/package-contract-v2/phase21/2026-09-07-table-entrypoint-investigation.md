# Table entrypoint recovery: missing generic member premise

The bounded investigation found no positive premise for accepting the refused
Table case. No acceptance rule, protocol, receipt, trust policy, or coverage
metric was changed. The original `./flex-render` catalog remains separate and
untouched. No additional complete row is claimed.

The [machine evidence](2026-09-07-table-entrypoint-evidence.json) follows the
original published case-set pointer and records both accepted `./flex-render`
artifact cases, their receipt and resolution identities, the installed
declaration hashes, and the exact refusal. The unchanged denominator is five
declared entrypoints; recovering the four executable entrypoints would still
leave `./package.json` under the existing metric.

## Exact blocker

The retained graph attempt for `@tanstack/solid-table@9.1.2`, Solid 1, prepares
eight root cases: `.`, `./flex-render`, `./experimental-worker-plugin`, and
`./static-functions`, each under `import` and `import,solid`. Native
certification refuses `./static-functions:cell_getIsAggregated`:

- Demand `sha256:03d3d72159688acd7359e3e36c35ffbe3f6b65fe11f61c0193606821978d41b9`.
- Artifact case `144530b0cfa749cbc00600d826e6d9794895e44d5737490569989a5f74843d88`.
- Family `recursive-value-shape`; signature alternative 0 lacks
  `column.table.atoms.grouping.get`.

The runtime implementation in `@tanstack/table-core` calls
`cell.column.table.atoms.grouping?.get?.()`. Its published declaration accepts
`Cell<TFeatures, TData, TValue>` with a generic `TFeatures`. The declaration's
`Atoms<TFeatures>` maps over `keyof TableState<TFeatures>`, and that state
depends on the selected features. The signature does not assert that grouping
exists for every permitted feature set.

An exact Type Facts export-signature query against the retained installed
declarations, with callable depth 5, reports `column.table.atoms` but no
`grouping` or `get` below it. This is not a depth-limit failure. A complete
prefix is not evidence for an absent descendant.

The independent TypeScript check uses the real published typings:

```ts
import type { Cell, TableFeatures, RowData, CellData } from '@tanstack/table-core';
export function check<TFeatures extends TableFeatures, TData extends RowData,
  TValue extends CellData = CellData>(cell: Cell<TFeatures, TData, TValue>) {
  return cell.column.table.atoms.grouping?.get?.();
}
```

TypeScript **5.9.3**, `--noEmit --strict --skipLibCheck --module esnext
--moduleResolution bundler --target esnext`, exits **2** with:

```text
check.ts(3,34): error TS2339: Property 'grouping' does not exist on type 'Atoms<TFeatures>'.
```

The source and full output are retained under `/private/tmp/table-published-types`.
This is evidence about the missing declaration premise, not a new checker
diagnostic or a claim of a runtime defect. The optional runtime call does not
authorize asserting unconditional callability in an accepted contract.

## Regression protection and next boundary

`mapped_signature_paths_test.go` checks a reduced feature-dependent mapped
signature against a concrete signature and a feature set without grouping.
Only the concrete signature supplies the exact required callable getter.
The Rust test `operation_path_requires_exact_member_not_only_closed_prefix`
checks that the verifier rejects a missing or unknown descendant, even under
a complete prefix, and accepts an independently stated exact callable fact.
The temporary real-package diagnostic test was removed after inspection.

Recovering this case would need a consumer-bound conditional member premise
or an independently proved narrower proposal. Substituting one chosen feature
set, broadening the published signature, ignoring the missing fact, or borrowing
another receipt would not prove the currently demanded claim. Those broader
changes are outside this bounded slice. General creates-refusal work remains
out of scope.

## Fresh retry and validation

The fresh pinned verifier built by `make verify` reproduced the same demand,
artifact case, family and missing path in
`rust/target/ecosystem-investigations/2026-09-07-table-recovery.json`. It
prepared all eight cases and then refused publication. The scoped recovery
attempt therefore has **certification status refused**, not a successful
partial-to-complete transition; the runner's headline "partial" describes
proposal generation and is not a certification verdict. No case-set catalog
was published by this attempt. The earlier two accepted `./flex-render` cases
remain unchanged in their separate original catalog. Do not enable recovery
for this row as a default replacement for its accepted partial lane.

- Focused Go test: generic, concrete and empty-feature subtests all passed.
- Pinned `make test-focused`: the exact verifier regression passed (one test,
  zero failures; `SOLID_TYPEFACTS_BIN` armed).
- Full main-worktree `make verify`: **exit 0, TOTAL 123.22s**, no
  `FAILED during step` marker; log `/private/tmp/table-main-verify.log`.
- `git diff --check` passed. No snapshots, public contracts, or production
  proof rules changed. No commit or push.

The full corpus was not repeated: this slice changes only regression tests
and documentation and establishes zero new certifications. The earlier
320-complete full measurement remains the last comparable corpus result.
