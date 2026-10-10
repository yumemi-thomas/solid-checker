# ADR 0135: An obligation in a private helper follows the call graph, and only an exact one

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the syntax fact (`rust/crates/solid-facts/src/ast/binding_references.rs`,
  `import_binding_references`), the call graph
  (`rust/crates/solid-reactive-ir/src/attribution.rs`, `import_binding_uses`,
  `reach_from`), and the reachability rung's guard
  (`rust/crates/solid-facts-backend/src/main.rs`,
  `function_published_by_its_module`, `imports_join_the_implementation`)
- Relation: this ADR adds to the existing `reachability` rung. It gives the
  rung one more starting point, and it closes a way the rung narrowed
  unsoundly.

## Context

After ADR 0134, six obligations of `@tanstack/solid-router@2.0.0-rc.8`'s root
node still marked all 97 exports. Each is filed at an import binding whose
uses sit only inside helpers that are not entry exports. For example,
`setupScrollRestoration` is used only in the private `useScrollRestoration`,
which `ScrollRestoration` and `useElementScrollRestoration` call.
Identity-widening finds no exported function around those uses. No reach
record existed for a binding outside every function body. So the ladder fell
to `fallback-all`.

Giving those bindings a reach record exposed a second defect. It was already
present, and hidden only because a sibling obligation marked everything. The
reachability rung attributed obligations in `Transitioner.js` and
`not-found.js` to **no export at all**:

1. The helper's module publishes it (`export { Transitioner }`), and another
   module imports it through `./Transitioner.js`, which the compiler resolves
   to `Transitioner.d.ts`.
2. That importer's binding therefore carries the declaration's symbol. The
   call graph walks the implementation's symbol, so it saw no caller and
   reported the enumeration complete.
3. The guard meant to catch this, `module_surface_is_unaccounted`, compared an
   export specifier's span with the declaration name's span. An export list
   (`function f() {}` … `export { f }`, which every bundler writes) never
   matches that way, so the guard read every such module as publishing
   nothing.

The pin below shows the effect: without the guard, a caller of such a helper
published `callbacks` **closed** over a dependency call that closes nothing.

## Decision

1. **An import binding's uses start the walk.** When an obligation is filed
   exactly at a value import binding, `import_binding_references` returns the
   binding's resolved Oxc references on the module's bytes. The walk starts
   from the outermost function around each use. If a use sits outside every
   function, such as a module-level read or an export specifier that
   republishes the binding, the enumeration is reported incomplete. It is also
   reported incomplete if the reference walk refuses the module (`eval`, a
   parse error). The rest of the walk is the existing one: its callers, and
   its escape test.
2. **Reachability never narrows through a helper that an importer the graph
   cannot see may enter.** A reaching function that is none of the
   entrypoint's exports, but that its own module publishes, makes the rung
   answer nothing, unless both of these hold:
   - every other module that reaches it binds the implementation's own
     canonical symbol at the importing binding;
   - none of those modules re-exports it, imports the module as a namespace,
     `export *`s it, or loads it dynamically.

   The check resolves relative specifiers by ESM's relative-URL rule, with no
   extension guessing. A relative specifier inside the package that does not
   resolve to exactly one file refuses, and so does a nonliteral load. "Its
   own module publishes it" now follows the exact binder edge from an export
   list's reference to the declaration, as well as a declaration export's name
   span.

## Soundness

(1) says only where the binding is used. Everything about who enters those
functions is the call graph's existing, fail-closed answer, and a module-level
use keeps the widest answer.

(2) is a narrowing of the rung, in the safe direction. The rung may certify
that export X does not reach a helper only if every way into the helper is a
call edge the graph enumerated. A cross-module entry whose importer binds a
different symbol is an entry the graph cannot see. Requiring the importer's
own binding to canonicalize to the implementation's symbol is exactly the
condition under which those call sites are the implementation's edges.

## Consequences

- Pinned by `scripts/contract-helper-reach-attribution.test.mjs`:
  - `private`: an entry-local helper using an open dependency export opens
    exactly its caller. The export beside it is described as it is with no
    dependency.
  - `split`: the helper is published by its module and imported through a
    `.d.ts` sibling. Its caller must not close `callbacks`.

  Each half was falsified separately. With (1) disabled, the unrelated export
  gets marked. With (2) disabled, both of `split`'s obligations go
  `reachability` to no export, and `caller` publishes `callbacks` closed.
- The contract corpus (107 fixtures) and coverage are unchanged.
- On solid-router's root node, replayed exactly:
  - `setupScrollRestoration` is now `reachability` to `ScrollRestoration` and
    `useElementScrollRestoration`.
  - Four obligations that c7e589ba attributed to no export
    (`Transitioner.js` 4009 and 4027, `not-found.js` 370 and 408) now mark
    every export instead.
  - Catch-all obligations: 6 before, 9 now, every one of them sound.
- The 9 left:
  - **`getStoreFactory` (2).** A private arrow in `routerStores.js`, whose only
    use is as an argument of `super(options, getStoreFactory)` into
    `RouterCore`. When it runs is the dependency's to say. It is exact only if
    `RouterCore`'s constructor contract states its `callbacks` for that
    parameter (invoked during the call, or retained by the instance). Then the
    obligation is construction or instance-member of `Router`, under ADR 0134.
  - **Component render (7).** `Transitioner` and `Rendered` are reached only
    through `createComponent(Transitioner, {})` in `Matches.js`. `getNotFound`
    is reached from `Match.js`'s callbacks and from `CatchNotFound`. Two
    things are needed: the `.d.ts` split joined, so that a
    `runtime-module-resolutions` feed or the ADR 0132 runtime join also covers
    importer bindings; and `createComponent(C, …)` treated as a call edge to
    `C`. That second step needs the dialect to state that `createComponent`
    invokes its first argument synchronously, on the caller's stack. The graph
    already treats a JSX tag as a call site.
