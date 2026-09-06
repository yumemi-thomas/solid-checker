# ADR 0037: Reproduction conditions for the pinned interpreter

- Status: accepted and implemented (2026-09-06); written with the
  implementation
- Date: 2026-09-06
- Owners: probe harness binding (ADR 0006) and the policy-2 gate schedule
- Relation: amends ADR 0006's "Export conditions" section and its `argv` and
  `conditions` rows; closes the `vetoUnreproducible` withholding ADR 0036
  introduced for a synthesized veto the pinned Node could not run for the
  artifact case. Sandbox scheme `scheme-version:10` → `scheme-version:12`
  (11 is the browser family's, ADR 0033).

## Context

ADR 0006 passes the artifact case's requested export conditions to the pinned
Node as `--conditions=` flags, asks the interpreter which set it actually
applies, and refuses at planning time unless Rust's own resolver, replayed
under that applied set, selects the very runtime target the Type Facts witness
read — for the root plan under every scheduled import kind and for every graph
dependency plan under an ESM import. That check is what turned the
2026-09-06 corpus's condition gap into named withholdings instead of false
passes.

The gap itself is one shape. Node applies `node` on its own, and every
`solid-js` and `@solidjs/web` in the audited corpus orders its `exports`
`worker, browser, deno, node, development, import`: a consumer's artifact case
selected under `[]` or `["solid"]` plus `default` reads `dist/solid.js` (the
`import` key), while the interpreter, walking the same object with `node` in
its set, stops at `node` and would load `dist/server.js`. The dependency node
fails the replay, and every synthesized veto whose closure carries that node is
withheld: 220 candidates on the 2026-09-06 run, plus 18 under `development`
(`dist/dev.js` against `dist/server.js`), across `@solid-primitives/form`,
`@corvu/drawer`, `motion-solidjs`, the TanStack query packages and the corvu
accordions.

Writing the tracer tests for this ADR exposed a second, worse shape of the
same gap. The replay covers *planned* cases — the root and, in the graph
lanes, every dependency node. A value-only transaction plans no dependency
node, yet its recipe runs the package's own top-level `import "solid-js"`
inside the private copy of the authenticated closure, and nothing compared
what the interpreter selected there with what the certified closure resolves.
The stub whose `server.js` answers the recipe differently proved it: the veto
ran against `server.js` with no refusal at all. Every value-only row in the
corpus whose closure carries `solid-js` therefore had its vetoes run against
Solid's server build — a weaker falsifier than the file the closure certified,
silently.

Node cannot be told to drop `node`. It can be given another condition, and
`browser` is ordered *before* `node` in exactly these manifests, so an
interpreter carrying `--conditions=browser` walks `worker ✗, browser ✓` and
lands on the `import` target inside the `browser` branch — `dist/solid.js`,
the certified file.

Two alternatives were rejected. Certifying the `node`-conditioned artifact
case instead, so the interpreter reproduces it, is unsound for the claim: the
server build of `solid-js` has no reactivity, so a `creates` veto against it
observes nothing about the client build the consumer runs. A checker-owned
resolver inside Node through module customization hooks reverses ADR 0006's
premise that Node's resolver is the arbiter and its echo the proof, and
duplicates the resolution premise the browser lane (ADR 0033) already owns.

## Decision

The gate batch runs a **bounded reproduction-condition search** before
anything is copied or launched:

0. The planning-time replay now covers **every accepted dependency edge** of
   the root plan's and every graph dependency plan's verified closure, in
   every lane (`refuse_unreproducible_dependency_edges`): the edge's
   entrypoint is resolved from the authenticated dependency snapshot under the
   importer's requested conditions — the set the closure was replayed under —
   and under the interpreter's applied set, and the two must name the same
   file. Rust against Rust, because no plan carries the dependency's expected
   path; the run-frame echo of every declared dependency resolution remains
   the independent half.
1. The requested set is tried first. When the replay passes for the root plan,
   every graph dependency and every closure edge, nothing is added. For a
   closure without a condition-sensitive dependency this is the whole pre-ADR
   behavior, unchanged.
2. Otherwise each entry of `REPRODUCTION_CONDITIONS` — today exactly
   `["browser"]` — is added to the requested set, the interpreter is asked
   what it applies under the resulting flags, and the condition is **admitted
   only when both hold**:
   - the replay passes under the resulting applied set, for the root plan
     under every scheduled import kind and every graph dependency plan under
     an ESM import (`refuse_unreproducible_artifact_case`, exactly as before);
   - every `exports` and `imports` object in **every manifest of the
     authenticated closure** selects the same target under the applied set as
     under the set the corresponding artifact case was selected under
     (`require_condition_neutral_closure`). Each plan's snapshot is compared
     under that plan's requested set; a closure snapshot no plan names is
     compared under the root plan's. The comparison is per scheduled import
     kind. Subpath maps and imports maps are walked key by key, pattern keys
     as written; a conditional object is resolved the way Node's
     `PACKAGE_TARGET_RESOLVE` walks it — first key that is `default` or in the
     set answers, a nested object matching nothing is backtracked over — and
     the two answers must be equal; an array keeps every member.
3. When no entry is admitted, the refusal is the requested set's own reason —
   the one the ecosystem runner's `vetoUnreproducible` bucket already reads —
   followed by what each attempt added and why it too was refused.

What the record carries:

- `EnvironmentIdentity::conditions` gains a third tag beside `requested:` and
  `esm:`/`require:`: `reproduction:<c>` for each admitted condition. The
  interpreter's applied sets already include it, because they are measured
  under the resulting flags.
- The bound harness identity — the receipt's probe-gate root — gains
  `reproduction-conditions:<comma-joined list>`, empty when nothing was added.
- The sandbox policy vector renames
  `argv:worker-path-plus-requested-conditions-only` to
  `argv:worker-path-plus-requested-and-admitted-reproduction-conditions-only`
  and adds
  `resolution:reproduction-conditions:browser,added-only-when-requested-set-does-not-reproduce-and-every-closure-manifest-selects-identically`;
  `scheme-version` moves from 10 to 12.

The private workspace passes the resulting set — requested plus admitted — as
its `--conditions=` flags. Nothing else about the launch changes.

## Why the added condition is safe to pass

A `--conditions` flag influences one thing: which file Node's resolver selects
for a conditional target. Every selection that reaches a certified claim is
verified — the planned entries by the replay, the subject's and the declared
dependencies' resolutions by the run-frame echo (ADR 0006 mechanism 3), and
now every other conditional target in the closure by the neutrality walk. The
private workspace holds only the authenticated closure, so there is no file
outside it a moved selection could reach. What the walk adds over the replay is
exactly the exposure the added condition creates: a subpath no plan names, or a
`#internal` import the package resolves for itself, that a `browser` key would
move onto bytes the witness never read.

The requested set is deliberately **not** held to the neutrality walk. The
interpreter's own defaults (`node`, `module-sync`, `node-addons`) are what
ADR 0006's per-node replay dispositions, and holding the base set to the walk
would withhold rows that certify today on a discipline this ADR does not
change. That exposure — an `imports` entry the interpreter's own `node`
moves — predates this ADR and is recorded under "What still refuses".

## What still refuses

- A closure whose manifests are not neutral under `browser` — a
  browser-conditioned `#import` or an unplanned subpath — keeps the
  `vetoUnreproducible` withholding, with the divergent key named.
- A dependency that orders `node` before `browser`, or that has no `browser`
  key leading to the certified file. The corpus has none today; the refusal
  names both attempts.
- `@tanstack/custom-condition` (12 candidates): not a plain condition name
  under the harness grammar, refused before the search runs. Widening the
  grammar to what Node's own flag accepts is a separate decision.
- The interpreter's own defaults moving an `imports` entry under the requested
  set alone: not new, not closed here.

## Consequences

- The 2026-09-06 corpus's `vetoUnreproducible` population — 220 `node` cases
  and 18 `development` cases — becomes runnable vetoes; whether each closes is
  then decided by the run, exactly as for every other synthesized veto.
- Every value-only row whose closure carries `solid-js` now either runs its
  vetoes against the client build the closure certified, with `browser`
  admitted, or withholds them with the edge named. Before this ADR they ran
  against the server build and reported a pass.
- Every receipt's policy digest changes with the scheme version, as ADR 0006
  requires of any disposition change.
- Pinned by `probe_harness::tests::the_neutrality_walk_admits_browser_only_where_it_moves_no_target`,
  `::a_conditional_leaf_follows_nodes_first_matching_key_and_backtracks`, and
  the three tracer tests
  `a_reproduction_condition_lands_the_interpreter_on_the_certified_client_build`,
  `a_reproduction_condition_that_moves_any_closure_target_is_refused`, and
  `a_reproduction_condition_that_does_not_reproduce_is_refused_with_both_attempts`,
  which run the real pinned interpreter against a `solid-js`-shaped stub whose
  `server.js` answers the recipe differently.
