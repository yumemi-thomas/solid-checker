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

`kind` is the one claim schema v1 has no unknown sentinel for, so a single
wrong one blocks its whole entrypoint from machine verification.

Expected generation:

| Export | `kind` | `callbacks` |
| --- | --- | --- |
| `DirectError` | `function` | `{"status":"unknown"}` |
| `BaseError`, `ChildError`, `Watcher` | `function` | `{"status":"unknown"}` |
| `AliasedWatcher` | `function` | `{"status":"unknown"}` |
| `plainFunction` | `function` | *absent* |
| `settings` | `value` | *absent* |

Three resolution shapes are pinned deliberately, because the corpus needs all
three: a class declared and exported in the entry file, a class reached
through a barrel's `export { … }` of an imported binding (the
`@tanstack/db` shape — the alias symbol's own declaration is the import
specifier, so the compiler's alias chain has to be walked), and
`const Alias = SomeClass` (the `@tanstack/db` `Query` shape). `settings` is
the negative: a real non-callable value stays `kind: "value"`.

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
