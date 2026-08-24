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
(`@tanstack/*`).

The exact fact that answers it is `Constructability` —
`GetSignaturesOfType(…, SignatureKindConstruct)` at the same span the callability
demand already resolves — never a name, a type text, or a syntactic search for a
`class` keyword. This fixture used to pin that search (a
class-expression-initializer fact on `BindingFact`, since deleted, plus a symbol
walk); it now pins that the
search is not needed, because the type answers every shape it recognized *and*
the two it could not: an IIFE-wrapped class whose initializer is a call, and a
class reached only as a tuple element type declared in another package.

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
| `./destructured`'s `inlineCacheName`, `dependencyCacheName` | object pattern over a class expression / a class identifier, both binding a `string` | `value` | *absent* |
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
  real values here, `./destructured`'s two strings, and the refusal on
  `./unresolvable`. It is the only site that can refuse, which is why the
  refusal is what this fixture pins there.

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

**Why `./destructured` is its own entrypoint, and what it pins now.** An object
pattern destructures a *member* of its initializer, so neither the class
expression (`const { name } = class Named {}`) nor the class identifier
(`const { name } = DependencyCache`) says anything about what the binding holds;
both hold a `string`. Under the retired syntactic class search that was
undecidable in both directions — following the initializer's symbol made each of
them a `function` claim about a value that cannot be called, and gating the
search off left `nonCallable` proving nothing, because `nonCallable` is what a
class type answers too. The entrypoint was therefore *refused*, and its two
correct `value` claims were the measured cost.

The constructability fact discharges it: the type answers the pattern directly.
`(class Named {}).name` is `nonCallable` **and** `nonConstructable`, which is the
full negative, so both exports publish `kind: "value"` and the entrypoint emits.
The same fact decides the shapes the refusal existed to protect — a static class
member (`const { Inner } = Container`) and a tuple element whose type is a class
(`const [Core] = pair`) are `Constructable` and raise to `function`. No syntax
participates in either direction any more, so this entrypoint is now the
*positive* pin on a destructured binding: it must publish two values, and a
regression that reinstated a shape gate would refuse them again.

**Why `./unresolvable` is refused rather than published.** `fromHost` is
`any`, so Type Facts answers `Callability::Unknown` *and*
`Constructability::Unknown` — the second fact reads the same type and fails
closed on the same flags, so it rescues nothing here: no closed domain, hence no
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

An export whose location carries *no* signature fact at all is refused too, on
the same reasoning. `demand_plan` requests both facts at every export specifier
and every exported declaration name, and an assertion in
`solid_facts_backend::semantic_demands`' tests pins that, so absence at one of
those spans is the producer finding no node to classify rather than the plan
declining to ask — missing evidence about this export, and `kind: "value"` is a
claim rather than a default. That arm is unreached by any fixture here and
pinned by unit test instead
(`export_kind_proof_tests::absence_on_either_fact_is_unanswered_not_a_negative`).

The residual hole is a different one: lib.es5.d.ts's signature-less
`Function`-supertype family answers both closed negatives while
`typeof x === "function"` can still hold at runtime, and nothing on this side
can detect it. `fixtures/package-contracts/function-supertype-kind` pins that
wrong answer deliberately. See docs/precision-backlog.md.


## What the closure record pins here

`expected-generation.json` names `index.ts` and `sibling.js` — and nothing under
`node_modules/`, although the analysis reads the installed dependency's own
artifact to decide the two kinds it carries across. That exclusion is the record's
scope rule (`packageScope` in packages/cli/scripts/generate-package-contract.mjs):
a dependency's bytes are not this package's bytes, no republish of this package
changes them, and hashing them would make the record depend on the install layout
and on the dependency's version — so two generations over byte-identical package
bytes would refuse to transfer a review. The dependency's own generated contract
and closure record are what describe those bytes.
