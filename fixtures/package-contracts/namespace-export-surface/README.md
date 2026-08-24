# Namespace export surface

Pins which names a `namespace` puts on a module's **export surface**: the
namespace itself, and none of its members.

A TypeScript `namespace` body is not module scope. `export const inner = 1`
inside one binds a property of the namespace *object*, so
`import { inner } from "namespace-export-surface"` does not resolve — `inner`
is reachable only as `Config.inner`. The contract generator published those
members as module-level exports anyway: `export namespace Config { export const
inner = 1 } export const real = 2` generated an entrypoint whose exports were
`["Config", "inner", "real"]`, and a consumer's import of `inner` was then
checked against a contract that claimed the package exports it.

Three independent paths produced the phantom, and this fixture covers all
three:

- the nested `export` statement is an `ExportFact` like any other, and the
  surface enumeration walked every one of them (`AstFacts::module_level_exports`
  is the filter now, at every enumeration site: `entry_export_entity`,
  `external_export_summary_for_file`, and `export_is_type_only` in
  `solid-facts-backend/src/main.rs`, and `resolve_named_export` and
  `contract_export_fragment` in `solid-reactive-ir/src/contracts.rs`);
- `AstFacts::exported_bindings` selected every declarator inside the outer
  `export namespace …` span, excluding only declarators inside a *function*
  body — a namespace body is not a function body, and neither is a **class
  static block**, so a declarator inside one leaked onto the surface the same
  way (`exported_bindings` now also excludes a declarator strictly inside a
  `ClassFact::span`);
- a **name collision** between a nested specifier and a module-level export of
  the same name: `file.ast.exports` is sorted by *span*, and a name-keyed
  enumeration that walks every export (nested ones included) can bind a
  module-level name to a nested specifier's type facts merely because that
  specifier's export statement sits earlier in the source. `helper` pins this:
  `internal`'s nested `export function helper` sits before the module-level
  `export const helper`, and before the fix `entry_export_entity` bound the
  module-level `helper` — a number — to the nested function's type facts and
  published `kind: "function"`.

What the entrypoint must publish:

| Name | Why |
| --- | --- |
| `Config` | the namespace object itself, a real module export |
| `Merged` | a class merged with a namespace: the class is the runtime binding |
| `settings`, `plainFunction` | negative controls, ordinary module exports |
| `helper` | a module-level `number`, decided by its own type facts, not the nested `internal.helper` function's |
| `Holder` | an ordinary exported class; its static block's declarator must not cost it its surface |
| `boxed` | a class-expression initializer whose own declarator span *contains* the class it initializes |

What it must **not** publish: `inner`, `Config.helper`, `Nested`, `deep`
(members of `Config`), `marker` (a static hung off `Merged` by the merged
namespace), `hidden` (a member of an unexported namespace, whose own `export`
keyword is the same trap one level further from the surface), `internal`'s
nested `helper` function (a member of the `internal` namespace object, not a
module export — the module-level `helper` is a distinct `number` binding of
the same name), `insideStaticBlock` (declared inside `Holder`'s static block,
not reachable from outside the class), and `hiddenB` (declared inside
`boxed`'s static block, for the same reason).

`Merged` also pins that the merge does not cost the class its kind: a class is
`typeof === "function"` at runtime, and the class declaration's name span is
what decides it (`AstFacts::declares_class_at`) — see
`fixtures/package-contracts/exported-class`. `Config` is a plain object, so it
is a `kind: "value"` row, decided by the two closed signature negatives at its
declaration name. `Holder` and `boxed` pin the same class-kind decision through
a static block: neither's own kind is affected by what its static block
declares, only by what the class itself is.

**Why `boxed` must survive while `hiddenB` does not, from the same class
span.** The exclusion tests containment in a `ClassFact::span`, and the two
declarators sit on opposite sides of it. `hiddenB`'s declarator span is
*strictly inside* the class body, so the class span contains it. `boxed`'s own
declarator span is the whole `boxed = class { … }` initializer — it *contains*
the class expression, the reverse relationship — so the class span does not
contain it, and `boxed` is never at risk from this exclusion. A module-level
declarator can never itself sit inside a class body, so this containment test
cannot misfire on one.

No stub typings are involved: every name here is declared in the fixture's own
source, so nothing in this fixture can be looser than a published package.
`namespace`, class/namespace merging, and class static blocks (ES2022, and
type-checked here under TypeScript's default settings with no `tsconfig.json`
needed) are ordinary type-legal TypeScript, and `tsc --noEmit --strict` is
clean on `index.ts` — this claim is about a *runtime* export surface, which no
type error covers.
