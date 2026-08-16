# Precision backlog

The analyzer's known approximations, recorded so each is a decision with an
owner rather than a rediscovery. Items live here when a fix is a *design
change* — it would move findings broadly and needs its own fixture-gated
change — as opposed to the bounded corrections that land as ordinary fixes.

Direction legend: **FN** — misses real defects; **FP** — reports correct
code; **Both** — either, depending on the code.

## Compiler-faithful heuristics (verified against the 1.x compiler, do not "fix")

These were flagged as suspect eslint-plugin-solid ports and have now been
verified against the **pinned 1.x compiler**
(`solid-1x-compiler@79b9b637`, byte-faithful to
`babel-plugin-jsx-dom-expressions@0.40.7`) — the parity target is Solid's own
behavior, not upstream's quirks. Each entry below matches the compiler, which
is why it stays.

- **`on*` event-name detection** (`upstream_compat/shared_reactivity.rs`,
  `solid1x_attributes.rs`): the compiler's attribute lowering treats *every*
  `on`-prefixed DOM prop as an event (`plan.key.starts_with("on")`,
  `to_event_name` = the suffix lowercased), so `once`/`only` genuinely become
  listeners for events `ce`/`ly` when function-valued, and statically-valued
  ones are frozen into the template as plain attributes — exactly what
  `v1/event-handlers` reports. Upstream's `/^on[a-zA-Z]/` is *narrower* than
  the compiler (`on-foo` is an event to the compiler but invisible to the
  rule) — a documented FN of a stylistic rule, not an FP.
- **ASCII-only element-name case classification**
  (`upstream_compat/mod.rs::is_lowercase_led`): Babel's `isCompatTag` is
  `/^[a-z]/`, so a non-ASCII-led tag compiles as a component reference. The
  checker matches the compiler.
- **Static `innerHTML` without children is silent**
  (`no-innerhtml`, `allowStatic` default) and **single-line
  whitespace-only children block `self-closing-comp`** — configurable
  stylistic leniencies matching upstream's option defaults; neither can
  produce a false positive.

## Resolved: false negatives closed 2026-08-16

- **Leaf-owner rules follow the dynamic extent through exact helpers**
  (`cleanup.rs::helper_forbidden_operations`). `onCleanup`/`flush`/primitive
  creation in a project function's *synchronous extent* (body minus nested
  function bodies) throws when the function is called from a leaf scope; the
  call site in the leaf callback is flagged, naming the helper
  (`LeafOwnerOperation::via`). Resolution is the exact TypeScript identity,
  transitive with a cycle guard. Remaining boundaries, deliberate: an
  unresolved/ambiguous/package callee contributes nothing (package behavior
  is the contract surface's), IIFEs inside a helper count as nested bodies
  (silent), and helper calls written inside nested functions within the leaf
  callback are not the leaf's synchronous extent (silent, correct).
  The leaf callback must also be a **function literal written directly in the
  owner's callback argument**: `createTrackedEffect(makeCallback())` evaluates
  its argument under the enclosing owner *before* any leaf scope exists, and
  `createTrackedEffect(wrap(() => …))` hands the arrow to an opaque wrapper
  that decides whether and where it runs — neither is proof, so both are
  silent (**FN**, deliberate). `fixtures/reactive-ir/leaf-owner/` pins the
  `onCleanup`, `flush`, and primitive positives, the transitive hop, both
  the block-bodied and the expression-bodied leaf callback, the nested-body
  and event-handler negatives, and both argument-position negatives.
  Cost, accepted: the helper traversal is redone per call site rather than
  memoized by callee symbol. Depth is capped at 8 with a cycle guard and the
  walk only starts for a non-primitive call inside a leaf callback, so the
  fan-out is small; memoizing it is open work.
- **`draggable={false}` on draggable-by-default elements** (2.0 catalog).
  The rc.0 runtime removes the attribute on `false` (RFC 07's remove half),
  and removal selects `auto`, which is draggable on `img` and `a[href]` —
  flagged with the `draggable="false"` fix hint; 1.x stringifies
  (`draggable="false"` works) and is deliberately unaffected. The `a` default
  needs a **proven-present** `href` — a JSX string or the bare spelling. A
  spread-carried one may not be there, and a dynamic `href={expr}` is removed
  by the runtime when `expr` is nullish, after which the anchor is *not*
  draggable by default; both stay clean rather than guessed (**FN**,
  deliberate). Every other element and the string spelling stay clean too.
  Pinned in the backend `jsx-correctness` fixture for both dialects,
  including the dynamic-`href` anchor.

## Resolved: upstream quirks that contradicted the compiler

- **`on:`/`oncapture:` duplicate folding is gone** (2026-08-16,
  `solid1x_syntax.rs::duplicate_slot`). Upstream folds `onClick`/`onclick`/
  `on:click`/`oncapture:click` onto one name and reports runtime-legal pairs
  as duplicates. The compiler lowers `on:evt` to a bubble
  `addEventListener`, `oncapture:evt` to a capture `addEventListener`, and a
  non-delegated plain `on*` to one listener per occurrence — all attach, so
  none of those pairs is dead code. `v1/jsx-no-duplicate-props` now reports
  event-shaped names only for proven single-winner slots: the delegated
  `el.$$event = handler` property write (later-wins) and the statically
  valued template attribute (first-wins, shared with `attr:`). No upstream
  parity case pins the folding, so the corpus is unaffected;
  `fixtures/reactive-ir/eslint-compat` pins both directions.
  The slot model is **DOM lowering, so it applies to intrinsic elements
  only**. A component's props are a plain object the compiler never lowers:
  there the slot is the key as written, so `<MyComp onSave={a} onSave={b} />`
  and `<MyComp on:click={a} on:click={b} />` are real later-wins duplicates
  (the slot model would have silenced both), while `onClick`/`onclick` and
  `attr:title`/`title` are distinct keys.
  The static-value half is a *node-kind* test matching the compiler's inline
  branch (`StringLiteral`/`NumericLiteral`): `{0x10}` and `{1_000}` freeze,
  `{-1}`/`{+1}`/`{NaN}`/`{Infinity}` do not.

  **Known inconsistency, not fixed here:** `v1/event-handlers` (SC8001,
  `solid1x_attributes.rs`) still answers the same "is this value static?"
  question with `str::parse::<f64>`, so it treats `{-1}` and `{NaN}` as
  static where `jsx-no-duplicate-props` now does not. That check is an
  upstream-faithful port and no corpus case separates the two; aligning it on
  the compiler's node-kind test is open work.

## Design-change candidates (open)

### `execution-map-incomplete` (SC9004) is unreachable from real source

Both dialect compilers emit every `jsx-expression` operation together with a
same-span region or callback role in every decision arm, and
`CompilerFacts::classifies` matches by span containment — so
`uncovered_jsx_expressions()` is empty by construction. The rule defends
against externally produced or partial compiler facts only, which is why no
fixture can pin it; if a third compiler adapter ever lands, that adapter's
tests are where this rule gets its coverage.

### Generic member dispatch is partially resolved

Direct generic calls, class methods, object-literal methods, exact resolved
member declarations, and structural calls whose formal receiver can be mapped
to every exact in-project call-site argument now participate in summaries. A
member call with multiple exact candidates is certified only when their
semantic read/callback/async summaries are equivalent and none has an
unresolved callback-contract obligation; missing, unresolved, or different
candidates remain fail closed. Remaining **FN** cases are exported structural
helpers with unseen external callers, computed members, and receiver
expressions whose TypeScript facts do not expose an exact value.

### A shorthand property's value is resolved only within its own file

TypeScript projects a shorthand property's *own* symbol at `{ pathname }` --
never the referenced value binding's -- so no TypeFacts entity, reference, or
declaration fact at that span identifies the value. The binder that builds the
Oxc AST facts does resolve that exact reference, and its answer is now carried
on `ObjectPropertyFact::shorthand_binding`; `interproc.rs`
(`binding_initializer`, `named_accessor`) reads the declaration from it instead
of matching the spelling within the enclosing function. That is scope-exact, so
the previous block-scoping hole is closed in both directions.

The cross-file gap is now closed for **named relative imports**: a shorthand
whose binder declaration is a named import specifier follows the relative
specifier to the exporting file — exact ESM resolution against the analyzed
file set, never the filesystem — and matches that file's exported declaration
in the accessor map exactly as the same-file arm does
(`interproc.rs::imported_accessor`). What remains fail-closed, by design and
in each case yielding no structured property:

- **an ambiguous relative specifier.** `./values` can name `values.ts`,
  `values.tsx`, or `values/index.ts`, and which one a bundler picks depends on
  resolution settings this pass does not model. When more than one project
  file matches, `relative_module_file` returns `None` rather than taking the
  first one enumerated — file order is not evidence, and a proven accessor
  claim sourced from the wrong module would be worse than no claim. **Pinned**
  by the fixture's `ambiguousShorthand` (`ambiguous.ts` +
  `ambiguous/index.ts`, both exporting the accessor).
- **bare and path-mapped specifiers**, which the resolver rejects outright
  (it only walks `./` and `../` against the analyzed file set, never the
  filesystem or `tsconfig` `paths`). *Not* pinned by a fixture case.
- **namespace and default imports, and re-export chains**
  (`export { x } from "./elsewhere"`): the join accepts only a named import
  specifier bound to a same-file export declaration. *Not* pinned by fixture
  cases; the guards are visible in `imported_accessor` and its
  `export.module.is_none()` filter.

What the fixture pins today is the same-file resolution set
(`scopedShorthand`, `unprovenShorthand`, `shadowedShorthand`,
`writtenShorthand`), the cross-file named-import join
(`importedAccessorShorthand`), the ambiguity bail (`ambiguousShorthand`), a
non-accessor import (`importedShorthand`), and a global (`globalShorthand`).

Two resolvers now answer "which file does this relative specifier name":
`interproc.rs::relative_module_file` and the backend's
`resolve_relative_export`. They are independently written and can drift.
Unifying them behind one owner is open work, deliberately not attempted in the
change that added the ambiguity bail.

## Partially resolved design changes

- **`v1/jsx-no-undef` now fails closed on missing semantic facts.** It reports
  unresolved `use:` names only when the structural binder proves that no
  lexical binding exists. Unresolved JSX tags, including dotted roots, are
  uncertifiable and silent. The old auto-import helpers remain test coverage
  for the upstream formatting logic, not a blanket semantic allowlist.
- **Unknown callback helpers remain contract obligations.** Exact TypeScript
  call facts now enrich the obligation and diagnostic with package,
  entrypoint, export/function, callback parameter index/type, required
  execution mode, and an editable schema-v1 contract stub. Standard-library
  behavior and project/package contracts can discharge it; unknown execution
  remains refused until an explicit contract proves it.

## Resolved design changes

- **Shorthand property values are resolved by the binder, not by spelling.**
  The value binding at `{ pathname }` is named by
  `ObjectPropertyFact::shorthand_binding` -- the declaration Oxc's scope tree
  chose for that exact reference -- so a same-spelled binding in a sibling
  block neither substitutes for the intended one nor makes it ambiguous. This
  replaced a spelling match scoped to the enclosing function, which both lost
  a provable structured return whenever any sibling block reused the spelling
  and, worse, could certify an accessor the shorthand never named. A shorthand
  the binder leaves unresolved carries no fact and proves nothing. The
  remaining cross-file gap is listed above.

- **Invoking a parameter's member is resolved per call site.** A function that
  calls `reader.read(value)` on its own parameter makes no claim about which
  implementation runs — that belongs to each caller. The owner records the
  obligation (`invoked_parameter_members`: parameter index and property), and
  `interprocedural_reads` resolves it against the argument actually passed at
  each site, the way `invoked_parameters` already substitutes a directly
  invoked parameter. A site whose argument is exactly one object contributes
  that object's reads; an unresolved argument, or a conditional over two
  objects, contributes nothing. This replaces pooling every call site into one
  summary, which made an unambiguous site uncertifiable whenever a sibling site
  was ambiguous. `fixtures/reactive-ir/interprocedural-methods-v2/` pins both
  halves: `invoke(objectReader, …)` reports at its own call span, while
  `invoke(cond ? objectReader : quietReader, …)` stays silent. The pooled
  `structural_parameter_member_symbols` path still supplies the function's own
  exported summary, where one answer must cover every call.

- **Callee resolution is exact and conservative.** Parenthesized, `as`,
  `satisfies`, and non-null wrappers are peeled through a shared AST fact
  helper. Resolved call declarations identify member callees when TypeScript
  provides them; static members can use their exact property entity, while
  computed members such as `handlers[i]()` fail closed instead of inheriting
  `i` or `handlers`.
- **Summary discovery covers method, alias, and returned-value branches.**
  Class/object methods, returned closures, conditional aliases, destructured
  function properties, and exact object spreads retain their canonical
  symbols. Direct generic calls and resolved structural member calls propagate
  summaries only through the dispatch proof described above; unresolved
  aliases and computed properties remain uncertifiable.
- **Transparent TypeScript wrappers are peeled at equality gates.** The
  shared helper is used by map/callback discovery, Solid 1.x structure gates,
  and shared reactivity function matching, with AST and fixture coverage for
  parentheses, `as`, `satisfies`, and non-null assertions.
- **Namespace-imported JSX primitives use dialect vocabulary.** `<Solid.For>`,
  `<Solid.Show>`, and `<Solid.Repeat>` resolve only when the namespace import
  is from a dialect-owned module and the member is in that dialect's export
  vocabulary. The namespace and named-import twins are pinned by
  `fixtures/reactive-ir/namespace-import-v2/`.
- **`prefer-component-syntax` covers conditional JSX returns and cross-file
  calls.** It follows exact TypeScript function identities, so lower-case
  value helpers and shadowed bindings stay out of the finding set. The focused
  `prefer-component-syntax-v2` fixture pins this branch for issue #210.

- **Component identity conventions are dialect-owned.** JSX call sites,
  direct JSX returns (Solid 2), and exact compiler-resolved Solid component
  aliases prove component identity. Solid 1 explicitly retains its upstream
  uppercase-binding convention for parity; the shared reactive core contains
  no hard-coded casing rule. Intrinsic-tag case checks remain syntax-only.
- **Dialect-owned type origin is no longer enough to register a source.** The
  dialect classifies exact exported aliases as component, accessor/resource,
  signal, store, setter, or store setter; user-local lookalike aliases and
  unrelated Solid types do not become accessors.
- **Unclassified function spans are `Unknown`.** Explicit compiler-untracked
  regions and other semantic proofs become `UntrackedRendering`; AST-proven
  module evaluation is its own one-shot role. Unknown reads/writes are not
  projected as violations.
- **Owner-shape recognition is AST-backed.** Binding immutability, array
  slots, call initializers, returned functions, and arrow kind now come from
  facts rather than scanning source bytes.
- **Compiler-established ownership is trace-backed.** Default compiler effect
  reruns emit typed owned regions without changing generated code. Custom
  wrappers make no claim; component and runtime-callback ownership still comes
  from exact TypeFacts identity and package contracts.
