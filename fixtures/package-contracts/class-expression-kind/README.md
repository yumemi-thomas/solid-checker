# Class-expression exports, and a kind no type answers

Pins the runtime `kind` of the class shape a *published* package actually
contains, and the one honest outcome when no closed type answers the question.

`fixtures/package-contracts/exported-class` pins the class *declaration*
shapes. This fixture pins the shape that survives a bundler: rolldown, esbuild
and tsdown all lower `export class C {}` to a `var` bound to an **anonymous
class expression** and re-export it by specifier. In that artifact no
`ClassFact.name` covers the exported binding, and `Callability` is truthfully
`nonCallable` — a class type has construct signatures and no call signature —
so the generator published `kind: "value"` and the runtime kind probe
contradicted it. That was 45 of the 53 failing `kind` claims in the corpus
measurement: `ReactiveMap`/`ReactiveSet`/`TriggerCache` (Solid Primitives),
`ResponseEnvelope` (`@solidjs/web`), `SelectionManager` (`@kobalte/core`), and
`AsyncBatcher`/`Debouncer`/`Queuer`/`Throttler` plus the `*DevtoolsCore` family
(`@tanstack/*`). The exact fact that answers it is the declarator's
class-expression initializer (`BindingFact::initializer_class`), never a name
or a type text.

Expected generation:

| Export | Shape | `kind` | `callbacks` |
| --- | --- | --- | --- |
| `LocalCache` | `const C = class {}` in the entry file | `function` | `{"status":"unknown"}` |
| `InlineCache` | `export const C = class {}` | `function` | `{"status":"unknown"}` |
| `SiblingCache` | barrel `export { … } from "./sibling.js"`, `var C = class {}` in a `.js` artifact with no `.d.ts` | `function` | `{"status":"unknown"}` |
| `DependencyCache` | bare-specifier `export *` into an installed dependency's own artifact, same lowering | `function` | `{"status":"unknown"}` |
| `siblingFunction` | plain function through the same barrel | `function` | *absent* |
| `dependencyFunction` | plain function through the same `export *` | `function` | *absent* |
| `settings`, `siblingTable` | real non-callable values, local and through the barrel | `value` | *absent* |
| `./destructured`'s `inlineCacheName`, `dependencyCacheName` | object pattern over a class expression / a class identifier | **entrypoint refused** | — |
| `./unresolvable`'s `fromHost` | re-export of an `any` | **entrypoint refused** | — |

The four positive rows reach the decision by three different routes, and the
attribution is worth stating exactly because it is easy to get wrong:

- `LocalCache`, `InlineCache` and `SiblingCache` are decided by
  `promote_callable_export` over **this** project's export map — the entry
  file's own declarator and the `.js` barrel's, both in files this project
  analyzes.
- `DependencyCache` and `dependencyFunction` are decided by
  `promote_callable_export` in the **dependency's own generation** (the
  generator recurses: `class-expression-kind` refuses at the bare-specifier
  `export *`, `bundled-dependency` is generated, and the parent retries with
  that contract), and then *carried* across the boundary by
  `external_export_summary_for_file`. Their kind is not re-decided here at all.
  That carried route is the safe flavour of carrying — the contract was
  produced by this run, from the dependency's own sources, under this same rule
  — and `fixtures/package-contracts/carried-value-kind` pins both it and the
  unsafe flavour it must not be confused with (an unreviewed contract merely
  found in `node_modules`).
- `promote_entry_callable`, the emission site, is reached for the summaries
  that are still `value` when the entry file's exports are assembled: the two
  real values here, and the refusals on `./destructured` and
  `./unresolvable`. It is the only site that can refuse, which is why the
  refusals are what this fixture pins there.

`function-2`'s and `function-3`'s summaries differ from `function-1`'s only in
key order because a carried summary arrives already serialized from another
document, and contract normalization keys a summary by its serialized form.
That is pre-existing normalization behavior, not something this fixture
asserts.

`sibling.js` carries no `.d.ts` on purpose. A declaration file would answer the
kind question through the compiler's alias chain and hide the defect, which is
why `@tanstack/pacer`'s `.` entrypoint (whose barrel imports resolve to
`batcher.d.ts`) was *already* correct while its `./batcher` entrypoint — the
same class, entered through the `.js` artifact — was not.

**Why `./destructured` is its own refused entrypoint.** An object pattern
destructures a *member* of its initializer, so neither the class expression
(`const { name } = class Named {}`) nor the class identifier
(`const { name } = DependencyCache`) says anything about what the binding
holds; both hold a string. This pins both directions of that:

- dropping the binding-shape gate in `identifier_binding_at` turns each of them
  into a `function` claim about a value that cannot be called;
- keeping the gate and then publishing the type's `nonCallable` answer as
  `kind: "value"` is the *other* wrong claim, because `nonCallable` is what a
  class type answers too — that is the whole reason `binding_declares_class`
  exists — and for a pattern the class search never ran. `const { Inner } =
  Container`, a static class member, is a constructor that invokes its callback
  and was published as the maximal certified negative. So a destructured
  binding whose kind is not otherwise provable refuses
  (`ExportKindProof::DestructuredMember`).

The cost is visible right here: these two really are strings, and refusing them
loses two correct `value` claims to avoid one wrong one. Separating them needs
a fact this analysis does not demand at an export-specifier span — the
constructability fact below, or `primitive_value_domain`. Recorded in
docs/precision-backlog.md.

**Why `./unresolvable` is refused rather than published.** `fromHost` is
`any`, so Type Facts answers `Callability::Unknown`: no closed domain, hence no
proof either way. A bare `kind: "value"` summary is the *maximal certified
negative* — `validate_export` bars a `value` summary from carrying even an
unknown claim domain, so it asserts "reads nothing reactive, returns nothing
reactive, invokes no caller-supplied callback, requires no owner". Publishing
that against no proof is what made `@solid-devtools/locator@0.16.7` certify
`addClickInterceptor(fn)` and `addHighlightingSource(fn)` as inert. `kind` has
no unknown sentinel in schema v1, so the honest outcome is the existing one:
refuse the entrypoint, keep the rest of the package, and let a consumer of
`./unresolvable` get an explicit uncertifiable result. The refusal is recorded
in the review plan; `expected.json` pins it by the entrypoint's absence.

One remaining hole, deliberately: an export whose location carries *no*
callability fact at all keeps today's `value`. `demand_plan` requests
callability exactly where it requests a type descriptor, so absence there is
missing evidence about the span rather than an answer about the type, and
refusing on it would refuse for a demand-coverage accident. See
docs/precision-backlog.md.

