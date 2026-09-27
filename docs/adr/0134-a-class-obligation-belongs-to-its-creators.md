# ADR 0134: A class obligation belongs to the exports that construct or hand out the class

- Status: accepted and implemented (2026-09-27); written with the implementation
- Date: 2026-09-27
- Owners:
  - the syntax fact (`rust/crates/solid-facts/src/ast/class_obligation.rs`,
    `class_obligation`)
  - the attribution ladder (`rust/crates/solid-facts-backend/src/main.rs`,
    `export_names_of_class_obligation`, `classes_stay_in_their_module`; the
    mechanisms `class-construction` and `class-instance-member`)
- Relation: comes after `reexported-import` (ADR 0133) and before
  `fallback-all`. It takes the owner decision of 2026-09-27 on instance-member
  obligations as its § 2.

## Context

After ADR 0133, `@tanstack/solid-router@2.0.0-rc.8`'s root node still had 15
obligations that marked all 97 exports. Nine of them sit in `route.js` and
`router.js` classes, and these are their shapes:

```js
var Route = class extends BaseRoute {            // heritage: import binding
  constructor(options) {
    super(options);                              // construction
    this.notFound = (opts) => notFound({ … });   // instance member
  }
};
function createRoute(options) { return new Route(options); }
var Router = class extends RouterCore {
  constructor(options) { super(options, getStoreFactory); … }
};
```

None of them sits in an exported function. The heritage and import-binding
obligations are at module level, and code in a class body is in no function
the ladder names. So every rung above `fallback-all` failed.

## Decision

`class_obligation` classifies an obligation against the module-level classes
of its module. A module-level class is `class C {}`, `export class C {}`, or a
`var`/`let`/`const C = class {}` declarator. The classification answers only
from resolved Oxc references on the module's bytes. It refuses on a parse
error, a semantic error, or any `eval`.

1. **Construction.** Each of these runs when an instance is constructed:
   - a bare-identifier heritage (`extends Base`), which reads a binding and
     calls nothing at module evaluation;
   - an import binding whose every use is such a heritage;
   - a constructor body outside the functions nested in it, `super(…)`
     included;
   - an instance field initializer outside nested functions.

   The obligation keeps all of its domains and is attributed to:
   - the entry names whose exact runtime binding (from the resolution record)
     is one of the module's export names for an affected class;
   - the exports the enclosing-chain rung gives each `new C(…)` site.
2. **Instance member (owner decision).** Each of these runs when a member of an
   instance is invoked later, not during the exporting call:
   - a method or accessor body;
   - a function created in the constructor or in a field initializer;
   - an import binding used only in such positions.

   The obligation opens only `returns`, and only of the same exact creators.
   An import binding that is used at both construction and member positions is
   treated as a construction obligation, whose domains are a superset.
3. **Affected set.** The class the obligation belongs to, closed over every
   module-level class that extends one of its members. Every resolved
   reference of every member must be one of:
   - a value specifier in the module's own `export { … }`;
   - the bare callee of a `new`;
   - the bare heritage of another module-level class.

   Any other reference refuses. That includes the class passed as a value,
   read, returned, or tested with `instanceof`.
4. **Package scope.** The backend resolves every analyzed file's static
   imports, `export … from` and literal dynamic loads with ESM's
   relative-URL rule, with no extension guessing. It refuses in each of these
   cases:
   - an edge that lands on the module and names an affected class, unless it
     is the entry file publishing it (an `export … from`, or an import it only
     re-exports, per ADR 0133);
   - a namespace import or an `export *` of the module;
   - a nonliteral load inside the package;
   - a relative specifier inside the package that does not resolve to exactly
     one file;
   - a `new` site that no export lexically contains.
5. **Refused shapes.** These keep `fallback-all`:
   - static members, static blocks and computed keys;
   - a nested class;
   - a heritage that is not a bare identifier, such as `extends mixin(Base)`,
     which runs at module evaluation;
   - for § 2 only, an instance that escapes inside its own class body: any
     `this` there that is not the object of a member access.

## Soundness

A module-level class value is reached only through its references. § 3
enumerates every reference in its module and § 4 every reference from the rest
of the package, so the exports that can construct it are exactly its public
names plus the exports containing its `new` sites. A subclass constructs its
base, so the subclass's creators are included. What runs at construction runs
only inside those exports' calls, or inside a consumer's `new` on a public
class name.

An instance member runs only on an instance, and an instance comes only from a
creator. § 5 refuses an instance that escapes locally. The owner decision is
that the fact belongs to what the creator hands out, which is its `returns`.

An instance can also reach a dependency base through `super(…)`. Whatever that
base does with it is described by the base's own contract. When that contract
is open, the heritage obligation marks every creator in every domain,
`returns` included. Any other retrieval of the instance goes through a
dependency call, and that call carries its own obligation.

## Consequences

- Pinned by `scripts/contract-class-attribution.test.mjs`:
  - an export beside the classes is described exactly as it is with no
    classes;
  - the export that constructs a class on an open base cannot close
    `callbacks`;
  - an export whose only open fact is an instance-member closure keeps
    `callbacks` closed and does not close `returns`;
  - an instance that escapes its constructor (`register(this)`) keeps marking
    every export.

  With the rung disabled, the first and third tests fail. The recognizer's
  unit tests use the `route.js` shape and pin each refusal.
- On solid-router's root node, replayed exactly, obligations that mark every
  export went from 15 to 6:
  - 8 are now `class-construction`: `RootRoute`/`createRootRoute`,
    `Route`/`NotFoundRoute`/`createRoute`, `RouteApi`/`getRouteApi`, and
    `Router`/`createRouter`;
  - 1 is now `class-instance-member`: `notFound`, attributed to `RouteApi` and
    `getRouteApi`.

  The viviana environment run is unchanged: 93 of 97 root exports are still
  degenerate. The 6 left are `getStoreFactory` in `routerStores.js` (2),
  `Transitioner` (2), `getNotFound` (1), and `useScrollRestoration` (1).
