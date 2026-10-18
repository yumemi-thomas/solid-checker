# ADR 0322: A static namespace member binds like a named import

- Status: accepted and implemented (2026-10-18).
- Owners: `namespace_static_members` and the namespace arm of
  `symbol_redirects` (`solid-facts-backend/src/source_redirects.rs`),
  `namespace_import_redirected` (`solid-facts-backend/src/package_requirements.rs`),
  the namespace arm of `callee_symbol` (`solid-reactive-ir/src/indexes.rs`),
  `AstFacts::type_names` (`solid-facts/src/ast/mod.rs`, facts schema 64).
- Relation: completes ADR 0313 § 3 ("fixing that needs namespace-member
  redirects") and narrows ADR 0290's `NamespaceValue` obligation.

## Context

`import * as SwitchPrimitive from "@kobalte/core/switch"` followed by
`<SwitchPrimitive.Root>` and `SwitchPrimitive.useSwitchContext()` is how most
Kobalte consumers import. Since ADR 0313 the package stays on source, but the
namespace's members are not redirected to the selected runtime exports, so the
import kept a `NamespaceValue` obligation and every use was `SC9005`. On the
49-project corpus (`/tmp/sweep-final13.json`) that is 8 of 17 `SC9005`, all
`@kobalte/core/*` (openbot, jibe-run, pekochan-blog).

## Decision

1. **Static members.** A runtime, non-type-only namespace import binding whose
   every reference other than its declaration is one of
   - the object of a non-computed member expression (`X.m`, `X.m(…)`,
     `X.m<T>(…)`),
   - the object of a JSX member tag or its closing tag (`<X.Root>…</X.Root>`),
   - inside a type query (`typeof X.m`) or a type-position name (a type
     reference `X.Props<T>`, a class `implements X.I`, an interface
     `extends X.I`; the new `type_names` fact),

   has each member redirected exactly as `import { m }` would be: the member's
   property span and the whole member (or tag name) span map to the selected
   module's export `m` (`symbol_redirects`).
2. **Obligation.** The import's `NamespaceValue` obligation is dropped only when
   every namespace binding of that specifier in that importer satisfies (1)
   *and* each member resolves to an entity whose canonical symbol a runtime
   redirect binds. One member left unbound keeps the obligation.
3. **Callee.** A namespace member call whose property entity is a redirect
   target is called by that target, not by its declaration-file symbol; any
   other namespace member call keeps its previous symbol (local namespaces are
   unchanged).
4. **Everything else refuses.** Any other reference (`const all = X`,
   `X[k]`, `f(X)`, `return X`, `{ ...X }`, runtime `typeof X`, `export { X }`,
   `import A = X.m`) leaves the import unredirected and its obligation in
   place. `export * as ns from "…"`, dynamic imports and `import =` stay the
   unsupported bindings `unredirected_packages` records.

## Proof obligations

| Claim | Premise | Refused by | Cases |
| --- | --- | --- | --- |
| "`X.m` is the export `m`" | ESM namespace objects are frozen views of the module's exports; a static key names one export | computed key; a member no runtime export carries (no redirect, obligation kept) | `Computed.tsx` |
| "No other code reaches the namespace" | every reference is a static member, a JSX member tag or erased | any value use of `X` | `Escaped.tsx`, `an_escaping_namespace_import_is_the_one_shot_answer` |
| "The member is not reassigned" | ESM namespace members are read-only; `X.m = …` is a `tsc` error and a runtime `TypeError` | n/a (not a case this checker reports) | |
| "A type name runs nothing" | type references, `implements`, interface `extends` are erased; `import A = X.m` is not recorded as one | runtime `typeof X` (not a type query) | `Frame` in `App.tsx`; `type_names_are_erased_but_import_aliases_are_not` |
| "The obligation is the import's" | all namespace bindings of the specifier in the importer | one unbound member or one escaping binding | `namespace_import_redirected` |
| "Package-internal namespaces are the same" | the redirect runs for every importer, project or selected source | an escaping namespace inside the package | `a_static_namespace_import_in_package_source_proves_its_call` |
| "A cached answer is current" | dependency-source checks are never served retained (ADR 0311) | n/a | `an_escaping_namespace_import_is_the_one_shot_answer` |

Self-review against the bug families found so far:

- **Key/receiver/origin identity.** The key is the member's source text at a
  non-computed property; origin is the import declaration resolved by Oxc's
  binder (`reference_declarations`), not the name. Receiver: `X.m()` passes
  the namespace object as `this` where `m()` passes `undefined`. This slice
  does not distinguish the two; a selected export whose body uses `this` is
  analyzed as if called bare. Recorded as a residual approximation.
- **Visible references only.** Every reference the binder records is
  classified; an unclassified one refuses the whole binding. A reference the
  binder cannot see (`eval`, `with`) is not possible in a module.
- **Script files.** Namespace imports exist only in modules.
- **Span equality.** Both the property span and the whole member span are
  mapped; the obligation check accepts a pair when either carries a bound
  entity, so a TypeScript entity located at either span is found.
- **Nested function confusion.** References are matched by their declaration
  span, so a shadowing parameter named `X` is a different declaration.
- **`__proto__`/`super`.** `X.__proto__` names no export: there is no redirect
  for it and the obligation stays.
- **Skippable awaits, primitive-by-cast, destructured parameters.** Not read
  here. A destructuring `const { m } = X` is a value use and refuses.
- **Merged alternatives.** `(c ? X : Y).m` is a value use of both and refuses.
- **Forwarding.** Passing `X` anywhere is a value use and refuses.
- **Retained staleness.** Redirects exist only under a dependency-source
  selection, which ADR 0311 never retains; the twin edits App between static
  and escaping use and back and compares with the one-shot answer.

## Consequences

49-project corpus against slice A (`/tmp/sc-opt/sweep-326f.json`, which equals
`/tmp/sweep-final13.json` on `SC9005`): **8 `SC9005` removed, 0 added**, 0
violations changed; 4,051 to 4,043 uncertifiable, 553 violations. Every
`@kobalte/core/*` `SC9005` is gone (dialog ×2, dropdown-menu, context-menu,
select, combobox, listbox, switch); the opened Kobalte sources raised no new
finding at any consumer site. The 9 `SC9005` left are `@kobalte/solidbase`
(5, kobalte-docs, not namespace imports), and 4 non-Kobalte packages. openbot
analysis wall 25.4 s to 25.2 s (noise). An earlier draft that preferred the
namespace property's symbol for every namespace call (not only redirect
targets) added 27 `parameter-default-unresolved` in ai-memory-ui's local
paraglide `m.*()` messages; restricting the callee arm to redirect targets
removed them.

Fixture `dependency-source-namespace-import` (new). Residual: a namespace
used as a value anywhere in its importer, a computed member, `export * as`,
dynamic imports and `import =` keep their obligations.
