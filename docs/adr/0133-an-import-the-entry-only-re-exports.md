# ADR 0133: An import the entry only re-exports belongs to the names it publishes

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners: the syntax fact (`rust/crates/solid-facts/src/ast/import_reexport.rs`,
  `reexport_only_import_names`), and the attribution ladder
  (`rust/crates/solid-facts-backend/src/main.rs`,
  `export_names_of_reexported_import`, mechanism `reexported-import`)
- Relation: the two-statement twin of the `reexport-specifier` rung. It comes
  after that rung and before `fallback-all`, and no earlier rung changes.

## Context

Bundlers write a cross-package re-export as two statements:

```js
import { replaceEqualDeep, SearchParamError } from "@tanstack/router-core";
export { replaceEqualDeep, SearchParamError, /* … */ };
```

When the dependency's contract leaves claims open for such a name, the
obligation is filed at the import binding. No function encloses the binding, and
its only references are specifiers in the export list, so none of the ladder's
rungs can answer:

- enclosing-chain;
- identity-widening, which finds no reference inside a function;
- reachability;
- `reexport-specifier`, which matches only `export { x } from "dep"`.

The ladder therefore marked every export of the entrypoint. After ADR 0132, 29
of the 44 remaining catch-all obligations of
`@tanstack/solid-router@2.0.0-rc.8`'s root node had exactly this shape.

## Decision

An obligation filed exactly at an import binding's local identifier is
attributed to the public names that publish that binding, when all of these
hold:

1. The binding is in the requested entrypoint's own entry file. A sibling
   module's export list publishes names the entry may rename or not publish,
   so it gets no answer here.
2. `reexport_only_import_names` proves from the entry's bytes, by Oxc's
   resolved lexical references, that the binding has at least one reference
   and that every reference is a value specifier of a module-level
   `export { … }` with no `from` clause. The module must also parse without
   error, have no semantic error, and reference no `eval`. A type-only import
   or specifier does not count.
3. Every name it returns is in this entrypoint's export map.

Otherwise the rung answers nothing, and `fallback-all` stays. Any other use
refuses, including:

- a call;
- a module-level read (`const held = x`);
- a class heritage;
- use inside a function.

## Soundness

This is the argument of the `reexport-specifier` rung applied to the other
spelling. An ESM import binding is immutable. If publishing it is the only
thing the module does with it, then the dependency's open claims reach
consumers only through those published names. Those names are described by the
dependency's own contract: the `dependency_bound` projection restores them
after attribution. No local body evaluates the binding, so no local export's
summary depends on it. An export that does use the name raises its own
obligation at the use, and the enclosing chain attributes that obligation.

The references are resolved lexically on the exact entry bytes. They are not
matched by name. `eval` is refused, because it could reach the binding
dynamically.

## Consequences

- Pinned by `scripts/contract-reexported-import-attribution.test.mjs`:
  - `twostep` (`import { clean, opaque } from "depkg"; export { clean, opaque };`
    beside a local export) must describe `local` exactly as `onestep`
    (`export { clean, opaque } from "depkg"`) does;
  - `held`, which also reads `opaque` at module level, must keep marking
    `local`.

  With the rung disabled, `twostep` fails. The recognizer's unit tests in
  `import_reexport.rs` pin the positive shapes (named, aliased, default and
  namespace imports) and the refusals.
- Measured on solid-router's root node, replayed exactly: catch-all obligations
  44 → 15, with 29 now `reexported-import`. The viviana environment run is
  unchanged: 93 of 97 root exports are still degenerate. The 15 left are
  module-level class heritage and constructor `super` calls in `route.js` and
  `router.js`, and references in private helpers (`getNotFound`,
  `getStoreFactory`, `useScrollRestoration`, `Transitioner`'s helpers).
