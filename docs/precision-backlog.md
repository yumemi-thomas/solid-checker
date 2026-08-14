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

What remains is a **FN**: the binder resolves an imported spelling to the
*import specifier in this file*, and both sites require the declaration to be
in the file being summarized, so `{ importedTracked }` naming an accessor
declared in a sibling module yields no structured property. Reaching it needs
the same cross-file declaration join the rest of contract generation already
performs, not a new fact. `fixtures/package-contracts/shorthand-block-scope/`
pins the resolved cases and this one.

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
