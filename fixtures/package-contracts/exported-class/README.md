# Exported class

Pins the runtime `kind` of an exported class, and the domain that has to fail
closed once it is stated.

**Constructability is not callability.** Type Facts derives `Callability` from
`GetSignaturesOfType(…, SignatureKindCall)` over the actual union
constituents; a class type carries construct signatures and *no* call
signature, so every exported class answers `nonCallable` there. At runtime
`typeof C === "function"` for every class — which is precisely what a
contract's `kind` describes and precisely what `contract probe`'s kind probe
measures (`typeof value`). The generator therefore published `kind: "value"`
for exported classes and the probe contradicted every one of them:
`@tanstack/solid-db@0.2.37` alone failed 102 `kind=value` claims, all of them
error classes.

`Constructability` — the same walk asked of `SignatureKindConstruct` — is the
missing half, and it is what decides `BaseError`, `ChildError`, `Watcher` and
`AliasedWatcher` here. Each is reached at an export *specifier* span, whose type
is the exported value, so `Constructable` answers directly.

**A class declaration is the shape the facts cannot be asked at**, in two
spellings — and neither is a single span, so nothing here says "the one span".
`export class DirectError extends Error {}` has no export specifier; the
exported name *is* the class declaration's name, and the compiler's type at a
class declaration name is the class's **instance** type — honestly `nonCallable`
and `nonConstructable`, because an instance is neither. The producer pins that
by test and its ADR 0020 says outright: demand at the export-specifier span,
never at a declaration name. So such a row is decided by the declaration it is
(`AstFacts::declares_class_at`), which is not a class-ness heuristic but a
span-addressing rule: `class C {}` binds the constructor by language
definition, and a bundler that lowers the declaration away leaves a *declarator*
name behind, which types as the constructor and is decided by the facts. This
fixture is the regression pin for exactly that row — wiring the two facts
without it published `kind: "value"` for `DirectError` while every other export
here stayed correct.

The second spelling is **anonymous**: `export default class {}` has no name to
record, so the export carries the `class …` node's own span, and the facts there
describe the instance type for the same reason. Reading them published
`kind: "value"` for a constructor — the one *false* maximal certified negative
this decision can produce, since `value` asserts both closed negatives. The
`./anonymous-default` and `./anonymous-extends` entrypoints pin both shapes
(bare, and with a heritage clause, which is what a published package actually
contains). `./named-default` and `./class-expression-default` ride along as
controls. The first was already covered by the name span. The second, `export
default (class {})`, is *not* a case `declares_class_at` stays out of: the
parser does not preserve the parentheses, so the export records the class
expression's own span — the same span `visit_class` recorded for it — and
`declares_class_at` matches it exactly as it matches the anonymous
*declaration* shape above. The match is redundant rather than load-bearing
here, because a class expression is a constructor by the same language
definition and ordinary `Constructability` already answers it correctly on its
own; the row is a control precisely because both paths agree.

`kind` is the one claim schema v1 has no unknown sentinel for, so a single
wrong one blocks its whole entrypoint from machine verification.

Expected generation:

| Export | Decided by | `kind` | `callbacks` |
| --- | --- | --- | --- |
| `DirectError` | the class declaration its name is | `function` | `{"status":"unknown"}` |
| `BaseError`, `ChildError`, `Watcher` | `Constructable` at the export specifier | `function` | `{"status":"unknown"}` |
| `AliasedWatcher` | `Constructable` at the export specifier | `function` | `{"status":"unknown"}` |
| `plainFunction` | `Callable` at the declaration name | `function` | *absent* |
| `settings` | both closed negatives | `value` | *absent* |
| `default` of `./anonymous-default`, `./anonymous-extends` | the anonymous class node the span is | `function` | `{"status":"unknown"}` |
| `default` of `./named-default` | the class declaration name | `function` | `{"status":"unknown"}` |
| `default` of `./class-expression-default` | the class expression the span is | `function` | `{"status":"unknown"}` |

Three resolution shapes are pinned deliberately, because the corpus needs all
three: a class declared and exported in the entry file, a class reached
through a barrel's `export { … }` of an imported binding (the
`@tanstack/db` shape), and `const Alias = SomeClass` (the `@tanstack/db`
`Query` shape). The last two used to need the compiler's alias chain walked by
hand; the type at the export specifier is transparent through an alias, an
import and a re-export, so both now answer with one fact. `settings` is the
negative: a real non-callable value stays `kind: "value"`.

**Why `callbacks` fails closed on a class.** The generator summarizes function
declarations, not construct signatures. Nothing in the summary carries what a
constructor — the class's own, or the one it inherits through `extends` — does
with the arguments a caller passes, and an omitted `callbacks` list is the
negative claim "invokes no caller-supplied function". A consumer reads
`new Watcher(onChange)` through exactly the same contract path as
`watcher(onChange)`, so publishing that silence would certify inertness
`Watcher` contradicts on its first line. The sentinel is demand-sensitive at
the consumer: constructing with no callable argument stays clean.

`Watcher` is in this fixture specifically so the case is a *proven* wrong
claim rather than a hypothetical one: its constructor invokes its argument.
