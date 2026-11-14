# leaf-owner-forbidden-call

`SC3001` · **error** · violation · 🛠️ safe fix available for trailing `onCleanup`

A call that requires child ownership or scheduler re-entry occurs inside a
Solid 2.0 leaf owner. The rule covers `onCleanup`, owner-attaching reactive
primitives, and `flush()` inside `createTrackedEffect` or an owner-backed
`onSettled` callback.

Written JSX counts as a creating call (ADR 0883): an intrinsic's dynamic child, attribute
or spread, a fragment's dynamic child and a component element, in the callback, its
parameterless IIFEs or a helper it calls.

## What it does

Leaf owners manage work through their callback's return value and cannot own
children. Consequently, `onCleanup` must be returned, derived primitives such
as `createMemo`, `createRoot`, and function-form `createSignal` cannot attach,
and `flush()` cannot re-enter the flush cycle. `Loading`/`Errored` called as
functions and `resolve`/`until` create an owner too (rc.17 only, measured; ADR
0614; on rc.21 their throw under `createTrackedEffect` is the observer guard's, so there they are
[no-await-primitive-in-tracked-scope](no-await-primitive-in-tracked-scope.md)'s, ADR 0641). Solid throws for all three
operation variants in its development build only: the `@solidjs/signals`
rc.17 production bundle (`dist/prod`) carries no such guard, so there the
call does not throw (ADR 0434). Value-form state such as `createSignal(0)` and
`createStore(object)` is allowed because it attaches no child work.

The callback must be a directly written function literal, an exact function
reference, or a closed local callback return. For the last form, the adapter
must have one unconditional return of a function literal or its exact callback
parameter; the returned function is then checked in its own synchronous extent.
A function named by value in the same file (a `const` or function declaration,
a chain of `const` copies, a receiver-closed object literal's data slot) is
walked as the literal is, each operation reported where it runs in the body,
once however many leaf calls take it (ADR 0918); one in another file is
reported at the argument.
Exactly resolved project helpers are followed transitively: a function or
`const` arrow, an import a path or alias resolves inside the program, and a
method declared in `C` of a `const` bound to `new C(...)`. Conditional,
wrapper-built, package, or otherwise unresolved callbacks (a slot written or
an object that escapes) produce `reactive-dispatch-unresolved` instead of being
guessed. A nested function
merely created by the callback executes outside the leaf extent, except a
callback a project helper calls in its own body (`run(() => createMemo(…))`,
through further such helpers, not in a parameter list or after an `await`):
it runs during the helper call, so an operation written in it is reported
where it is written (ADR 0492). A helper that defers, stores or hands on its
callback is not followed. A tagged template's tag (``f`x` ``) is followed as
a call of `f`, resolved through its declaration (ADR 0538).
`(function* () { … })().next()` runs its first step in place, as an IIFE; U
where a `yield` may stop it first (ADR 0929).

The callback of a primitive the dialect says runs in place is such a layer too
(`Dialect::slot_run`, ADR 0944): `untrack`, `latest`, `isPending`, `flush(fn)`
and `runWithOwner(owner, fn)` keep the leaf as the owner, so `untrack(() =>
onCleanup(…))` is reported at the `onCleanup`, in the leaf's text, in a helper's
body, or in a function named by value (`untrack(cb)`, reported at that call).
Tracking is off in `untrack` and `runWithOwner`, so `resolve` and `until` there
create a root under the leaf (this rule's, not the observer guard's). A call
that takes what its callback throws (`isPending`) leaves a plain throw
(`flush()`, the observer guard) unreported, while a cleanup or a creation the
leaf refuses emits its diagnostic first and is reported. The throw it takes is
the one a callback proven synchronous makes: an `async` callback, named or the
function a callback calls, throws a rejection the call does not catch, so its
`flush()` is an uncertifiable row at the call (`leaf-owner-unproven`), never a
violation. `runWithOwner(null |
undefined, …)` and an owner captured in a component body have no leaf owner
(silent); the `getOwner()` of the leaf's own function is the leaf's (reported);
any other owner value, a primitive attributed without the binder's
confirmation and a callback the walk cannot read are uncertifiable.
`createRoot` is refused at its own call and its callback is not crossed, nor is
a computation's or a deferred one's.

An action called in that extent runs its generator's first step there, under
the leaf owner (ADR 0520): a `const`-held action's body is followed as a
helper's is, up to its first `yield`. A site after a yielded promise runs with
no owner (`missing-owner`'s claim); one after a plain `yield`, on a branch, or
in an action no `const` here holds is an obligation; `flush` in the step throws
`FLUSH_IN_ACTION` first (`flush-in-action`'s claim) (ADR 0550).

A standard-library call in that extent is judged by its exact declaration,
optional (`el?.scrollIntoView()`) or not (ADR 0439). An inline callback
(`forEach`, the `Promise` executor) is guaranteed only where its carrier
provably runs it: over an empty or unknown-length receiver (`items().forEach`,
a `NodeList`) the finding is uncertifiable (ADR 0563). One that dispatches an
event synchronously (`focus`, `click`, `dispatchEvent`, dialog and popover
toggles, `AbortController.abort`, and since ADR 0705 `form.submit`,
`reportError`, `replaceWith`, `replaceChild`, `replaceChildren`,
`History.go`, `Location.assign`/`reload`/`replace`) keeps the obligation,
since a listener it reaches may call `onCleanup`; so does an optional
`getContext`.

`onSettled` becomes a leaf owner only when its call is proven owner-backed.
Out-of-band calls enqueue a plain callback and remain outside this rule; an
unprovable caller or nullable owner is uncertifiable. An `onSettled` called inside
another leaf body is always queued out of band, because that body's owner is
children-forbidden: it is never claimed here, and its callback is judged as
out-of-band (`missing-owner`).

An `onCleanup` in an owned leaf is this rule's claim: the runtime throws
`CLEANUP_IN_FORBIDDEN_SCOPE` there, and it does so wherever the cleanup sits in
the leaf's extent, a helper the leaf calls included. A `createTrackedEffect`
body runs under the tracked node whatever the effect's owner, so its cleanup is
this rule's claim even when the effect itself has no owner. The same site is not
also reported as `missing-owner` (SC4001), which claims only a cleanup that never
runs: one in an `onSettled` callback queued with no owner, or with an owner that
may be absent (ADR 0347).

The operation's identity is proven from the binder (ADR 0711): a named import
whose imported name is the primitive's, or a member of a namespace import of a
Solid module. Any other attribution (a renamed re-export through a project
barrel, an import whose target the producer could not read) is uncertifiable.

A function entered twice under different contexts is walked twice (ADR 0961): the walk's visited set is keyed by how a
body is entered (the entry kind, whether it is guaranteed, the argument standings the call proves, the class `this` is
exact for), so `make(true); make(false)` reads the second entry and `isPending(g); g();` reads the direct call after the
one `isPending` catches; a slot callback is walked on a copy, so its visits never suppress a later direct entry. A cycle
still ends where a body is entered again the same way.

A call written in a callback of a primitive that runs it in place, where the callback takes a parameter, is async or is a
generator (`untrack((p) => onCleanup(…))`, `untrack(async () => { createMemo(…); })`), is an uncertifiable row of this
rule (ADR 0959, the `UnmodelledSlot` operation, blocker `leaf-owner-unproven`): the slot climb does not cross such a
callback, and what it runs is not read. A call that cannot operate is not claimed (a primitive the leaf owner never
refuses, an accessor read, a setter call); any other call is, a helper or a member too. Never a violation.

A call of a generator whose iterator the leaf callback keeps (`.next()`, a `for…of`, a spread, `Array.from`, a stored loop,
`yield*` through another generator, an async generator's `.next()`) is an uncertifiable row of this rule (ADR 0961, the
`GeneratorStepped` operation, blocker `generator-stepped`): a generator's body runs where its iterator is stepped, which no
walk here follows, so a forbidden operation in a body that holds a call may run in this leaf scope. A call a statement
discards runs none of the body, and a generator with no call in its body has nothing to run.

A parameter initializer a leaf callback's call runs (a default, a destructuring default, an IIFE or closure in one, a
constructor's) is covered by the withholding rows of the call (`leaf-callback-opaque`, `value-as-parameter-default`), as a
function the walk cannot follow is; a `new` call's row stands at its `new` token, where V8 places the frame (ADR 0961).

A site no execution reaches is uncertifiable (ADR 0458). Not yet: a site in a
tracked-effect body whose every creation is inside an owned leaf body, where
the creation itself throws first (`docs/open-work.md`).

## Examples

Incorrect:

```tsx
function Widget() {
  onSettled(() => {
    onCleanup(dispose);
    const label = createMemo(() => count());
    flush();
    console.log(label());
  });
  return <div />;
}
```

Correct:

```tsx
function Widget() {
  const label = createMemo(() => count());
  onSettled(() => {
    console.log(label());
    return dispose;
  });
  return <div />;
}
```

## Code run without a call (ADR 0963)

A leaf callback runs a member without a call: a property access runs a getter, a coercion `toString`, `valueOf` or
`[Symbol.toPrimitive]`, a `for...of` or a spread `[Symbol.iterator]`, a Proxy access its trap, `new` a constructor and the
class's field initializers, a tagged template its tag. The walk follows calls and runs none of these. A creation, `onCleanup`
or `flush()` written where such a member runs it (a class element, an object literal's accessor or method, a tag function, a
function a `new` runs, a helper those call) is an uncertifiable row of this rule (`function-run-unplaced`;
`field-initializer-unplaced` directly in a class field's initializer), never a violation, while the program names a
primitive that opens a leaf scope. A function handed where nothing follows it is run by a call, which the walk answers
itself (SC9012, `leaf-callback-opaque`): not this net's. The net adds no row where the walk's own operation stands (a helper's creation is reported at the helper call), and an opaque call's SC9012 row answers for no accessor or method written in its arguments.

A function stored under a key the host reads by itself is such a route however it is written (batch 10c): a method, an arrow, a
`function`, a name bound to one or an assigned member under a coercion key (`toString`, `valueOf`, `toJSON`), the promise or
iterator protocol (`then`, `next`, `return`, `throw`), `handleEvent` or a computed key no literal decides; a trap of a `new Proxy`
handler; the `get` or `set` of a descriptor handed to `Object.defineProperty` and its siblings. Any other key runs by a call.

## How to fix

Return cleanup from the leaf callback; a trailing `onCleanup(fn)` has a safe
rewrite to `return fn`. Move owner-attaching primitives to the component body
or another children-capable scope and read their accessors inside the leaf.
Remove `flush()` because leaf callbacks already run during settlement, or move
the write and flush together to the event handler or imperative boundary that
triggered the work.

## Accounting (ADR 0966)

Every flush the flush census lists ends, for this code, in a row of this rule at the site, an SC9012, the compiler's discard, or a typed reason (the program names no primitive that could put the flush where this code applies, or the flush's caller is on an empty stack: a scheduled callback, code past an `await`, module evaluation, a test runner's callback written directly, or a site nothing enters). A flush none of those ends gets an uncertifiable `SC3001` row, `leaf-operation-unaccounted`: the walks that place a flush also answer "placed off" for a function no call edge reaches, a body that may be a component and an owner set around the call, none of which is a proof, so those sites stay uncertifiable (never a violation). The program-wide reason (the program names no primitive that could put the flush where this code applies) is not a proof for a flush in a function an installed package the build does not read as source may run: the package calls the primitive itself, on a function the project hands it (`runStep(() => { flush(); })`), and nothing in the build's table shows it. Such a flush gets no program-wide reason and keeps its `SC3001` row, `leaf-operation-unaccounted`: a function written as an argument or inside one (an object's member, an array's element), a reference to it through any copy, assignment or import, a JSX prop, spread or child of a component the program does not read, a project function's parameter that goes on to one of these, a function a project function returns (`pkg.run(make())`), a reflective call that forwards its arguments (`pkg.run.call(null, cb)`, `Reflect.apply`, `bind`), and a helper the handed callback calls (ADR 0967). A callee is known, and so exempt, by its resolved declaration: a dialect primitive, a reviewed test runner's registration, the standard library, a project function, a DOM element, or a package the build reads as source; `expect`, `vi.*` and `waitFor` are not exempt. The default is the flip's (the review of #216): a value counts as handed unless a fact of the program proves where it goes, so a function kept in a collection or an iterable's element, an assignment into an object a package exports or `this`, a class's functions, a project component's props that go on to the package, `<Dynamic component={…}/>` of a package's component, a function a handed function returns, a callback run inline by known code (the standard library, a project parameter, a dialect primitive) on the stack of a step a package runs, a reflective call whose target is a project function that goes on, a namespace that escapes whole and an export of an open program keep the row; a value that is only called, compared, registered with the host (`addEventListener`), assigned to a local element or handed to a project function that does none of these is not handed. A program that installs fake timers (`vi.useFakeTimers()`, `jest.useFakeTimers()`: a test advances the clock on the stack it is on, which may be an action's step) has no scheduled-callback reason either: a flush written in the callback of `setTimeout`, `queueMicrotask` or `Promise.then` keeps its row of each code (F1 of the review of #213, ADR 0967; the clock is global to the run, so the question is the program's). A test runner's callback, the code past an `await` and module evaluation are unchanged.

## Endings of the owner-call sites (ADR 0969)

Every call of `onCleanup` and of a creation a leaf owner refuses (`createMemo`, `createEffect`, `createRoot`, `render`, `resolve`, a function-form `createSignal`, ...) is a site, for CLEANUP_IN_FORBIDDEN_SCOPE and PRIMITIVE_IN_FORBIDDEN_SCOPE, and so is a JSX creation (ADR 0971: a component element, a tracked child, attribute or spread, a fragment's dynamic child), which creates a root, a render effect or a memo as a call does; the same silences, declines and gate answer it. It ends, for its code, in a row of this rule (the leaf walk's operation, or the member-route net's), an SC9012, the compiler's discard, or a silence that names the fact the throw needs and the code lacks; with none it gets an uncertifiable `SC3001` row, `leaf-operation-unaccounted`, never a violation. A creation written in a component element's children or in a prop's value (a JSX creation or a creating call; ADR 0971, the review of #225, F2) is a getter that runs where the component reads the prop, so nothing the function that holds it says places it: it ends only when it never runs, and otherwise gets the code's uncertifiable row (decline `getter-runs-where-read`); the getter of a component that renders it in place (the dialect's `Show`, `For`, `Index`, `Repeat`, `Switch`, `Match`, `Loading` and `Errored`, a context provider) creates under the owner that render sets and is no site. A function the element hands over whole (a prop's value that is an arrow, a function child) is a plain site; a function the getter's own evaluation calls (a `.map` callback, an immediately invoked arrow, an `untrack` callback) is the getter's. An SC4001 obligation at the same call does not decline this rule's code. The silences: the site never runs (ADR 0458); it is at module scope (no function, no instance field's initializer: module evaluation runs it once on a stack of its own); the project names no primitive that opens a leaf scope (the census includes a package the build reads as source); or the owner graph gives the owning function a context with an edge, no leaf bit and not the bare open-world seed of an export, and the site is plain (not in a parameter list, after an `await`, or in a generator; no layer around it entered by a reference that is no direct call: a function that returns JSX is entered by renders alone only where every reference of its binding is a JSX tag, so a component a variable holds and a leaf owner's callback calls keeps the row; the review of #220, F1). The census and the graph answer only for a function no package the build does not read may run (ADR 0967: such a package may open the leaf scope itself, or call the function under one): a component only the router package renders, a function handed to a package, an export of an open program keep the row until a proof from the package's source says where they run.

## Related

- [missing-owner](missing-owner.md) — operations with no owner at all
- [primitive-in-directive-application](primitive-in-directive-application.md) — owner-attaching work in a directive application callback
- [reactive-dispatch-unresolved](reactive-dispatch-unresolved.md) — callback bodies whose exact execution cannot be certified

ADR 0967, review causes A–E: containment alone does not place a callback. A receiver inside an initializer, an iterable, a template part or an operand of `yield` or `await` stays handed; a project function naming `arguments` hands every argument, and a callback's returned value follows its receiving call whether it is given literally or by an exact current named reference (an unresolved member remains uncertifiable). An intrinsic element's attributes other than compiler-recorded event handlers, its children and its spreads (also `Dynamic` with a string tag) run on the creating stack. Opening and closing JSX names ask the same render owner, and code in an element's prop or child expression (a nested element, a call, an inline callback) runs where that element evaluates it: under a component nothing reads, when the package reads the prop; a project component's props and an object literal's getter run where they are read, and a method call on them binds `this` to the object. `this` and `super` name an object no reference of it shows: a value placed directly in an object literal that writes one (a method, a getter, a function it names, a prototype, a spread's members) is handed, as is a prototype installed by a write of `__proto__` or `prototype` (a `const` literal that writes no receiver takes the write as a `__proto__:` key). A function stored under a key a receiver reads (`this.k`, `super.k`, `const { k } = this`; any key when the receiver is used as a value or the key is not named), in code a package may run, is handed unless its holder provably cannot be a receiver (a plain `const` literal every use of which keeps it so, and what those uses give out asked in turn): one assigned after the literal, a derived literal, an `Object.create` result, a class's instance. In a program that can name `Object.prototype` (a route to it under any name, a read under a key no expression proves, code it builds as a string, an installed package whose closure names it, builds code or is not read to its end) no holder is proved plain, and where code is built as a string or a package is not read to its end any key is read through any receiver: a function stored in an object keeps its row. A key is the property key the runtime gives it (a unicode escape is decoded, `0x1` is `1`), a parameter that is a pattern or a rest reads every key where its function runs, and `Object.assign(this, …)` is a write of any key. A function stored through a parameter's member (`t.m = f` in `function put(t) { … }`) is stored into the object the callers pass: it is followed to each caller's argument, under the name written at the call, when every use of the function is a direct call the build resolves, and handed otherwise (an alias of the object made before the call, a copy of the parameter, a default or a spread argument, a method of an object literal and a call from another file are not followed: `docs/open-work.md`). An assignment expression's value is followed like any value. Unproved hand-offs retain the uncertifiable row.
