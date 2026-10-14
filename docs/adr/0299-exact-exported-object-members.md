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

## Review round 1

Two false-violation paths, both fixed:

- A namespace exposed the receiver without an identifier reference to it.
  String access in project source and dot access through a dependency barrel
  escaped the census. Module namespace, dynamic-import, require and namespace
  re-export channels now withhold literal targets, including through named and
  star barrels. Runtime redirects join declaration and implementation references.
- A closure called a function-local receiver before its initializer. Its Get
  throws, so the original method never runs. Local uses now require the same
  function and a position after initialization; captures and switch-containing
  functions are refused. Module uses require a function, no static source cycle,
  and no unmodeled top-level call before the initializer.

The base SC1001 at reader.readCount predates this commit's owner edges. It came
from the symbol-based method-summary path, which followed the selected property
declaration without receiver closure. Both that path and older accessor transport
now respect the shared literal target census; callback timing's older local
method fallback also refuses withheld targets. Unknown callback arguments retain
their obligations. The direct dependency namespace reproducer was already
refused by ADR 0297 selection; an export-star barrel reproduces both violations
with dependency source selected. No selection or package-integrity code changed.

Fixtures: object-member-namespace-escape, dependency-source-object-escape and
object-member-initialization. All pass TypeScript 5.9.3 against real published
Solid 2.0.0-rc.13 typings. The same-function initialized control remains a
violation; withheld targets stay silent or retain definition-site obligations.

Validation: 379 IR library tests; coverage 291 projects / 2,012 findings;
ownership gate 42 cases, zero pending rows; formatting, diff, schema, dialect
manifests and IR Clippy (all targets, warnings denied) pass. Three snapshots are
new; four existing snapshots lose two read proofs and demote two callback reads
whose older paths bypassed an existing shared veto. The final release/browser
sweep (`experiments/sweep-sol-r0299.json`) has 49 projects, zero errors and
562 violations. `diff.jq` / `comm -3` against sweep-0297r.json has no rows, so
there are no moved violations to hand-review. Uncertifiable findings are
3,248 → 3,211. Full make verify and workspace-wide Clippy were deferred for
this focused IR change; no contracts or manifests changed.

## Limits

This is one literal receiver level. Structural types, let bindings, namespace
receiver chains, conditional/nullish initializers, extracted functions,
unresolved identities and arbitrary object-producing calls remain refused.
Refusal supplies neither an owner edge nor a new read summary; existing
definition-site obligations remain.
Namespace uses are withheld wholesale, including harmless reads: no namespace
use census is claimed. An unresolved load withholds module-scope literal targets
throughout the analyzed source set. Barrels conservatively include every import
of an exporting file. Static cycle checks use the analyzed source graph and
exact declaration/runtime redirects; missing project-module edges and modules
depending on a cyclic module are refused. Unfollowed external implementations
remain outside that graph. The switch and receiver-inertness spelling vetoes may
also withhold on comments or strings. Function-local captures stay refused even
when a particular caller runs them after initialization.
