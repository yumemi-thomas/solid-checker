# ADR 0134: A class obligation belongs to the exports that construct or hand out the class

- Status: accepted and implemented (2026-09-27); written with the implementation;
  amended 2026-09-28 (a member construction can invoke is construction)
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

## Amendment (2026-09-28): a member construction can invoke is construction

§ 2 assumed that an instance member runs "not during the exporting call". That
is false for a member the construction itself invokes.
`@tanstack/router-core@1.171.22`'s `RouterCore` constructor installs
`this.update = (newOptions) => { … }` and then calls `this.update({ … })`, so
everything `update` does runs inside every creator's own call. § 2 opened only
the creators' `returns` for it, which is unsound in the direction that
publishes a closed domain. The amendment makes three changes.

1. **Construction includes the members it can invoke.** Construction code is
   every constructor body and instance field initializer, on the class and on
   each of its module-level bases. A member is one of these:
   - a method or accessor;
   - a closure the constructor installs as a top-level `this.key = <function>`;
   - a field whose initializer is a function.

   A member is *reached* when reached code names its key on `this` or `super`.
   That covers `this.m(…)`, `this.m.call(this, …)`, a read that hands the
   member on, and a setter write. The member's body is then reached code too,
   so reach is transitive. Keys are matched by name across the hierarchy,
   which can only over-approximate what runs. An obligation in a reached
   member is a construction obligation, in every domain, of the same exact
   creators.
2. **Any other closure the constructor or an initializer creates is
   construction.** § 2 used to classify it as an instance member. A callback
   handed on (`list.forEach(() => …)`) or an IIFE can run during construction.
3. **Fail closed where the reached set is not exact.** The class rung answers
   nothing, so the obligation stays `fallback-all`, when any of these holds:
   - a class on the way has a base that is not a module-level class (an
     imported base's constructor may call any member, an override included);
   - `this` or `super` is accessed with a computed non-literal key;
   - an instance escapes as a value (`this.m.call(this, …)` and
     `.apply(this, …)` are the only exceptions);
   - a callable member is written anywhere other than its own installation, or
     is declared twice, or shares its key with an accessor or a data field;
   - a class has a static member or a static block;
   - a reached key is defined by two classes in the hierarchy (an override a
     base constructor would run in place of its own method).

   Prototype augmentation outside the class body is already refused by § 3.

The same shape exposed a defect in the `reachability` rung, which runs first.
The call graph bounds a function's callers by its references. A class method
is entered by member dispatch: a base constructor's `this.init()` names the
base's `init` and runs a subclass override that no reference names. The rung
therefore reported the override complete with no caller, and attributed its
obligation to **no export at all**. A class method is now never
"entered only through calls" (`compute_entered_only_through_calls`,
`solid-reactive-ir/src/attribution.rs`), and its obligations fall through to
this rung. Object-literal methods keep the reference test. They can also be
dispatched through a value typed by another declaration, and that remains
open.

### Consequences of the amendment

- Pinned by the `class_obligation` unit tests. Each of these fails on the
  previous rule:
  - `a_member_the_constructor_invokes_runs_at_construction`: the `RouterCore`
    shape, directly and transitively, through `.call`, a field initializer,
    and a handed-on closure;
  - `a_base_constructor_that_calls_an_overridden_member_refuses`;
  - `a_member_of_a_class_on_an_imported_base_refuses`;
  - `an_inexact_member_set_refuses`.
- Pinned by `scripts/contract-class-attribution.test.mjs`:
  - `invoked`: `createInvoked` cannot close `callbacks`, and `local` is still
    described as it is alone;
  - `inherited` and `overridden` keep marking every export.

  The previous binary fails both new tests. Without the reachability change,
  `overridden`'s obligation goes to no export.
- On solid-router's root node, replayed exactly, catch-alls went from 5 to 6.
  `RouteApi`'s `notFound` (`route.js` 465) was `class-instance-member`. Its
  class extends the imported `BaseRouteApi`, whose constructor this package
  cannot see. There are still 91 degenerate exports, and router-core's own
  node is unchanged. Coverage (127 fixtures) and the contract corpus (107
  fixtures) are unchanged.
