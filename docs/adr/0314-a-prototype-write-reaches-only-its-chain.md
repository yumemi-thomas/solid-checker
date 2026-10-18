# ADR 0314: A prototype write reaches only its chain

- Status: accepted and implemented (2026-10-17).
- Owners: `SemanticLookup::assigned_members`, `prototype_owner`,
  `host_prototype_owner`, `member_may_be_reassigned_on_builtin`,
  `member_may_be_reassigned_on_class`, `member_value_symbols_in` and
  `factory_member_value_symbols` (`solid-reactive-ir/src/indexes.rs`); their
  consumers in `interproc.rs` (ADR 0211 site), `cleanup.rs`
  (`exact_instance_method`), `execution_role.rs`
  (`member_runs_declared_method`) and `attribution.rs`
  (`value_holds_builtin` and its parameter/prop arms).
- Relation: narrows ADR 0209's prototype veto; tightens and extends ADR 0211's
  per-call-site member resolution.

## Context

ADR 0209 vetoes an exact member proof when an assignment writes a member of
that name, or when *anything* writes through a `prototype` under a key it
cannot name. That second clause was one project-wide flag. In app-game it was
set by two vendored writers -- ecsy's `Component.prototype[key]`, where
`Component` is a local function-expression constructor, and spector's
`object.prototype[originFunctionName]`, reached only as
`OriginFunctionHelper.storePrototypeOriginFunction(HTMLCanvasElement | OffscreenCanvas, …)`
-- and in derp-media-server by a test's `ElementShim.prototype = …`. Each
vetoed every exact-class and every built-in-value member proof in its program,
although none can reach those objects.

Separately, `member_value_symbols` searched for a property anywhere inside the
object's span: inside a call's arguments (`wrap({ run })` read as the result's
`run`), inside a nested literal, and through `let` and destructured bindings.

## Decision

1. **The census records whose prototype a write reaches.** A write through
   `R.prototype[k]` (any key) or `R.prototype = …` is narrowed when `R` is
   - a class, a function declaration, or a `const` bound to a function or
     class expression, declared in the writing file and never rebound: that
     constructor (its instances and subclasses' instances);
   - a parameter (identifier, no default, never rebound, no `arguments` in
     the function) of a static class method, when every reference to the class
     past its declaration and imports is the object of a non-computed member
     access, every member access spelled with the method's name anywhere is a
     direct call through the class, no literal computed key or destructuring
     names the method, and every call passes at that parameter a global
     identifier that no project binding declares, whose declarations are all
     ambient (in declaration files no analyzed source is) and include a `var`
     in a TypeScript default library: those host constructors. A host is
     *DOM* when every default-library `var` of it is in `lib.dom.d.ts`. The
     proof is dropped if the method or a host global is ever written.
   Anything else keeps the project-wide veto.
2. **Consumers ask about their receiver.** An ADR 0211 `Builtin` origin
   (literals, primitives, reviewed ECMAScript value classes and their fresh
   results), a fresh object literal, and a value proven to hold one are
   reached only by a non-DOM host write: no ECMAScript value inherits from a
   DOM interface or from a project constructor. A project class instance is
   reached by a non-DOM host write, or by a write to a constructor in its
   chain; an `extends` this cannot resolve counts as reached. Callers whose
   receiver is neither (`local_access`'s store-key exemption and its digest
   census) keep the old project-wide answer.
3. **Member resolution is exact** (`member_value_symbols_in`): only a `const`
   naming its whole initializer is followed; the object must be exactly an
   object literal; only its own properties and spreads count; a getter or any
   computed key resolves nothing.
4. **Factory results** (`factory_member_value_symbols`), for the per-call-site
   parameter member resolution only: a plain call of one exact, current,
   synchronous project function every return of which is an object literal
   written in the return (or a concise arrow body that is one); each literal
   must resolve the member; no assignment may write a member of that name; no
   reflective definition may use that key or one it cannot name.
5. The narrowed census is bound into the cross-file proof digest.

## Consequences

49-project RC.13 corpus: 46 SC9012 uncertifiable findings removed (app-game
39, derp-media-server 5, kui 2), none added, 553 violations unchanged.
Fixtures: `prototype-write-narrowed`, `prototype-write-object-host`,
`prototype-write-escaped-helper`, `factory-member-dispatch`. On the
pre-change binary `factory-member-dispatch`'s `ArgumentLiteral` was clean: the
old span search read the argument's property as the result's.

Not decided here: `Object.setPrototypeOf`, `Object.assign` onto a resolved
receiver and dynamic-key writes that are not through a prototype stay outside
the census, as before (ADR 0215). A direct `x.method()` callee does not follow
factories: doing so surfaced one new uncertifiable read in kui (`alerts.start()`
reaching an untimed `PromiseLike.then`) and was left out. A property whose
value is an arrow is not resolved by the summary lookup.
