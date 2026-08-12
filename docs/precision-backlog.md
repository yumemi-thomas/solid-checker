# Precision backlog

The analyzer's known approximations, recorded so each is a decision with an
owner rather than a rediscovery. Items live here when a fix is a *design
change* — it would move findings broadly and needs its own fixture-gated
change — as opposed to the bounded corrections that land as ordinary fixes.

Direction legend: **FN** — misses real defects; **FP** — reports correct
code; **Both** — either, depending on the code.

## Deliberately upstream-faithful (verified, do not "fix")

These were flagged as suspect heuristics and verified against
eslint-plugin-solid 0.14.5 (`6d3bc311`), the parity baseline. The checker
matches upstream on purpose; changing them is a parity divergence to declare,
not a bug fix.

- **`on*` event-name detection** (`upstream_compat/shared_reactivity.rs`,
  `solid1x_attributes.rs`): `starts_with("on")` plus an alphabetic third
  character is upstream's own `/^on[a-zA-Z]/`. `once`/`only` props qualify
  under both implementations.
- **`on:`/`oncapture:` folding in duplicate-prop detection**
  (`solid1x_syntax.rs::normalize_prop_name`): upstream lowercases every
  `on*` name and collapses both namespaces onto `on`, so
  `on:click` + `oncapture:click` *is* a duplicate upstream.
- **ASCII-only element-name case classification**
  (`upstream_compat/mod.rs::is_lowercase_led`): upstream's
  `isDOMElementName` is `/^[a-z]/`; a non-ASCII-led tag is a component under
  both.
- **Static `innerHTML` without children is silent**
  (`no-innerhtml`, `allowStatic` default) and **single-line
  whitespace-only children block `self-closing-comp`** — both match
  upstream's exact conditions (`isHtml` + children check;
  `childrenIsEmpty || childrenIsMultilineSpaces`).

## Design-change candidates (open)

### Name-case conventions as ownership/identity proofs — the big one

Component and hook identity is decided by identifier casing at many gates:

- `interproc.rs` `enclosing_render_function` gate on summarized reads: reads
  reaching a lowercase-named caller (a `useThing` hook, a module
  initializer, a factory) are never reported. **FN, project-wide.**
- `local_access.rs` / `owners.rs` (`inside_lowercase_named_function`,
  `inside_unclassified_callback`): accessor calls and props reads inside
  lowercase-named helpers or unclassified closures are suppressed unless a
  callback role was proven. **FN** — the mirror of the gate above.
- `owners.rs::seed_name_contexts`, `source_discovery.rs` props roots,
  `static_rules.rs`, `reachability.rs` roots: uppercase-led ⇒
  owner-providing/component. An uppercase factory (`Vector({x, y})`) is
  assumed owned (**FN** for owner rules, **FP** for SC1003); a lowercase
  component gets neither props-source nor owner treatment (**FN**).

Why deferred: casing is the load-bearing convention the whole owner model
seeds from; replacing it means deriving component identity from usage (JSX
call sites, `Component` type facts, compiler execution facts) and would move
findings in every fixture. Worth a design doc before any code.

### Callee resolution falls back to the smallest contained symbol

`indexes.rs::callee_symbol`: when neither the callee span nor its member
property carries an entity, the smallest symbol-bearing entity *inside* the
callee answers. `handlers[i]()` can resolve to `i`; `wrapper.value()` to
`wrapper`. Consumed by `local_access.rs` and `interproc.rs`. **FP** (phantom
reads/writes/actions attributed to proven sources). A fix must distinguish
"no resolution" from "wrong resolution" without losing the legitimate cases
the fallback exists for (parenthesized and cast callees).

### Type-origin registration of accessors

`source_discovery.rs` registers every TypeScript entity whose type
originates from a dialect-owned module as an accessor, picking the
declaration by alias-name text (`"Accessor" | "Setter"`). A
`Component`-typed value or a user alias named `Accessor` becomes a reactive
source. **FP.** Also positional: the second binding of a destructured pair
is assumed to be a setter. Fix needs real type identity (which export the
alias resolves to), not name text.

### Unclassified execution spans default to `UntrackedRendering`

`execution_role.rs`: spans the execution map did not classify take the exact
role the strict-read rule reports. Every compiler-fact gap becomes a
user-facing warning instead of a suppressed unknown. **FP.** The
alternative — an explicit `Unknown` role that reports as uncertifiable
rather than violation — is a diagnostic-model change.

### Functions invisible to the summary graph

`interproc.rs`: an AST function with no byte-offset-matched TypeScript fact
is dropped from summaries (class members, object-literal methods, offset
drift), and generic or non-identifier callees (`helper<T>(sig)`,
`obj.helper(sig)`) produce no edges or contract reads. **FN.** Needs either
sturdier fact matching or a conservative "unknown callee" edge.

### TS wrapper expressions defeat span-equality gates

No shared unwrapping of `as` / `satisfies` / non-null / parenthesized
expressions exists; classification gates that demand span equality
(`source_discovery.rs` mapArray params, `owners.rs` owner-providing regions,
several upstream-compat sites) silently reclassify
`createRoot((() => {...}) as VoidFunction)`. **Both.** A `peel_ts_sugar`
helper applied at each gate is bounded per-site but each site moves
findings, so it should land gate-by-gate with fixtures.

### Byte-scanning helpers in `owners.rs`

`go_binding_pattern_accepts_call` / `go_returned_arrow_pattern_accepts`
scan raw source: nested generics (`Foo<Bar<T>>`), defaults containing `)`,
and comments defeat them. **Both.** Now pinned by unit tests (including the
known-wrong cases, marked as such); a fix should come from AST facts rather
than smarter scanning.

### JSX member tags are not resolved against the vocabulary

`<Solid.For>` / `<Solid.Repeat>` after `import * as Solid from "solid-js"`
produce no control-flow classification: the JSX resolution paths match plain
tag identifiers against imports and declarations, never member expressions.
The call-form namespace vocabulary was widened to the census invariant
(`namespace_import_primitives`, pinned by
`every_modelled_export_resolves_through_its_namespace_module` in both dialect
modules), so this is now the only way a namespace import hides a primitive.
The silent shape is pinned in `fixtures/reactive-ir/namespace-import-v2/` —
false negatives for any rule keyed on control-flow or boundary tags when the
component arrives through a namespace member tag.

### `execution-map-incomplete` (SC9004) is unreachable from real source

Both dialect compilers emit every `jsx-expression` operation together with a
same-span region or callback role in every decision arm, and
`CompilerFacts::classifies` matches by span containment — so
`uncovered_jsx_expressions()` is empty by construction. The rule defends
against externally produced or partial compiler facts only, which is why no
fixture can pin it; if a third compiler adapter ever lands, that adapter's
tests are where this rule gets its coverage.

### Report-on-missing-fact in `v1/jsx-no-undef`

`solid1x_undef.rs` reports "not defined" whenever the demand plan produced
no entity for a span — any demand-plan gap becomes a hard violation. **FP.**
Also, the auto-import allowlist covers `Show/For/Index/Switch/Match` only.
