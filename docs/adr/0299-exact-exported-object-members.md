# ADR 0299: Calls of a stable exported object's own function

- Status: implemented (2026-10-14).
- Owner: `object_members`, shared by the owner graph, execution-role lookup
  and interprocedural read summaries.

## Context

ADR 0290 refuses the receiver symbol of a member callee: `f.reset()` does
not run `f`. That also leaves `timer.useTicker()` without an owner edge when
`timer` is a named import of a package's const object. Its reactive reads
need the same exact function target.

## Decision

Resolve the plain identifier receiver to its exact const declaration and
object initializer, optionally wrapped in the standard library's
`Object.freeze`, whose own visible replacement also vetoes the wrapper. An
anonymous default object uses its exact export-assignment/expression
declaration. Select one noncomputed own method, inline arrow/function value,
or shorthand declared function. The literal must be closed: no spread,
computed/numeric key, duplicate, getter, setter or prototype-setting property.

Join every configured-file binder and semantic reference of the receiver,
including exact named/default import and export channels. Every value use
must be an immediate noncomputed member call of an inspectable function that
cannot expose this receiver, including sibling shorthand methods. Writes, aliases, opaque arguments,
extracted members and namespace receiver chains refuse the target. The same
ADR 0273 written-name/wildcard index vetoes the selected member; ADR 0284's
wildcards stay scoped to their creating package, while named writes stay global.

The target is a source function location, not the composite callee's entity.
Both owner passes use it. Dependency owner attribution retains ADR 0290's
completion and caller-context checks. Reads use the existing transported
function summaries. The target map participates in retained owner identity
and interprocedural reuse; removing an admitted edge cannot reuse its result.
An anonymous member's read can keep its proof through ordinary synchronous
helpers when an exact local runtime source is its origin. Conditional and
detached async hops remain uncertifiable. A followed implementation no longer
also reports the obsolete type-only candidate at that call.

Optional calls are admitted only when the exact literal receiver and own
function prove both links non-nullish. Generators are refused. Functions
that might expose the receiver via `this`, `arguments`, `super` or `eval` are refused;
the conservative source spelling veto also refuses backslashes and may withhold
on comments or strings. Visible eval withholds this resolver's targets. It supplies no symbol identity. Unpatched built-ins
and ADR 0273's visible-write premise retain their existing scope.

## Evidence

- `object-member-import`: owner propagation and reads across project files,
  method/arrow/function/shorthand/default/freeze shapes, redundant optional links,
  and mutation/escape/namespace/nullability controls.
- `dependency-source-object-member`: package owner attribution, package reads,
  same-package wildcard refusal, and other-package/project wildcard scoping.
- `object-member-freeze-write`: replacing Object.freeze through Object.assign
  supplies neither the original method's owner edge nor its read summary.
- All fixture projects pass `tsc --noEmit` with faithful stubs and real
  published rc.13 typings. Base/freeze sources also pass the oracle in strict
  and loose modes. IR library tests: 379 passed.
- `typed-accessor-roots` gains two proven `tick` reads (direct/helper) and
  loses the obligation on `external.orientation`, whose imported literal
  holds a plain string-returning function. No other existing fixture moved.
- Coverage: 288 projects / 2,009 findings; three new snapshots and that one
  existing snapshot only. Ownership gate: 42 cases, zero pending rows. The
  touched crate passes Clippy with warnings denied; formatting, schema,
  dialect manifests and diff checks pass.
- rc.13 browser corpus, release binary: 49 projects, zero errors; 562
  violations and 3,287 uncertifiable findings, identical at every site to
  `sweep-m-all.json`. `diff.jq` / `comm -3` has no rows. There are no added
  violations to hand-review. Result: `experiments/sweep-sol-members.json`.

## Limits

This is one literal receiver level. Structural types, let bindings, namespace
receiver chains, conditional/nullish initializers, extracted functions,
unresolved identities and arbitrary object-producing calls remain refused.
Refusal supplies neither an owner edge nor a new read summary; existing
definition-site obligations remain.
