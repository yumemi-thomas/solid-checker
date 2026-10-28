# no-store-mutation-outside-setter

`SC2003` · **warning** · violation

A store or projection is written through its read proxy instead of through its
setter.

## What it does

**Props (2026-10-19).** A write through a 2.0 props object, or a `merge`/`omit`
view of it, is an uncertifiable result, not a violation. The compiler hands a
component a plain object (a write to a new key succeeds, a write to a
getter-backed key throws), and `omit` of it is a plain copy; only a proxy
(`merge`, split props) drops the write. Which one the object is, is not proven
at the write. Store writes are unaffected.

Flags a write through a proven store's read proxy, or a `createProjection`
result, without going through its setter. The former name
`no-direct-mutation` stays an alias, so an existing configuration keeps
working (ADR 0425). The shapes, each proven at the exact store or projection
symbol:

- an assignment, compound assignment or update (`store.a.b = x`,
  `store.n++`);
- a `delete` of a bare or parenthesized static member path (`delete
  store.a.b`) whose property Type Facts proves optional and not readonly
  (ADR 0461), its links past the root judged as an assignment's (ADR 0465,
  below); a missing fact makes it uncertifiable;
- a call of one of the nine in-place `Array.prototype` methods (`push`, `pop`,
  `shift`, `unshift`, `splice`, `sort`, `reverse`, `fill`, `copyWithin`) on a
  path of the store. The callee must resolve through Type Facts to the
  standard library's `Array` method, so a project method of the same name, a
  typed array's `fill` and an unresolved receiver are silent (upstream's
  `no-store-mutation-outside-setter` matches the method name);
- the same writes through a `createProjection` result, reported as a
  projection write (it has no setter to name).

**Through a hook result (ADR 0482).** A member read that is exactly the store
(`const holder = useState(); holder.state`, ADR 0481) is the write's root, so
`holder.state.v = 1` is reported like `state.v = 1`, and a write inside the
store's own setter called as `holder.setState(…)` is exempt.

**Past the root (ADR 0465, 0476).** A write that reads no link past the root
(`store.v = 1`, `list.push(1)` on a store that is the array, a cast root) is a
violation: `createStore` wraps its initial value whatever it is
(`dist/dev.js:5904-5915`), so the root is always the read proxy. A write through
a link (`store.a.b = 1`, `store.items.push(1)`) is uncertifiable, except that
under the program's `strictNullChecks` (Type Facts reports it as the compiler
resolves it) a link whose type may hold `null` or `undefined` is TS18048/TS18047,
so the rule is silent. The proxy serves a child only through `isWrappable`
(`dist/dev.js:2272-2283`), which refuses a frozen object, a non-plain prototype
(a `Map`, a class instance) and a DOM node and serves it raw: a write below such
a link lands (Chrome, witnessed with `Object.freeze`). Whether a link's value is
plain is a property of the value, not of its type, so no fact proves the link is
wrapped (ADR 0476). Fixtures `store-write-chain-links`,
`store-write-chain-links-loose`, `store-setter-extent`.

The runtime drops all of them outside a setter: the proxy's `set` trap
returns `true` without storing (`@solidjs/signals@2.0.0-rc.17`
`dist/dev.js:5691-5700`), and an in-place array method reaches it once per
element and for `length`. Probed on rc.17: `delete`, all nine mutators, and a
write to a projection result outside its compute function are dropped
(STORE_WRITE_OUTSIDE_SETTER, `:5420-5455`); the trap for `delete` is
`:5761-5767`. TypeScript accepts the mutators and projection writes on rc.17
(`Store<T> = T`; a nested array is mutable on every release), which the
tsc oracle checks against the published typings.

Two writes are exempt because rc.17 commits them (probed, with the value read
after `flush()`). Each holds while a window is open, a dynamic extent rather
than a region of text (ADR 0476):

- A write through the original store proxy **while one of that store's own
  setter callbacks runs**: `storeSetter` adds the store to `writeScopes` for the
  callback (`dist/dev.js:5847-5866`), so the trap's `inDraft(target)` holds
  (`:4955-4957`). `setStore(draft => { store.value = 7 })` commits like a draft
  write. The pairing is proven, not guessed: the callee must resolve to the
  setter destructured beside that exact store, the write's root and the callee
  through exact `const` aliases (`const view = store`, `const set = setStore`). Writing through *another*
  store's proxy inside a setter is dropped (a different `scopeKey`) and stays a
  finding, as does any write outside a setter. Upstream reports the own-setter
  write; that divergence is pinned as ownership case
  `no-store-mutation-outside-setter/own-setter-write/001`.
- A write through a projection's result symbol (or a function-form
  `createStore(fn, seed)`'s store) **while its compute function runs**:
  `runProjectionComputed` runs the function inside `storeSetter(draft, …)`
  (`dist/dev.js:6675-6678`). Ownership case `…/projection-compute-write/001`.

The window is open on the callback's own stack: the callback itself, an IIFE, an
inline standard-library callback (`forEach`) and a project helper that calls its
callback, crossed as the write rules cross them (`layer_run`). A write the
callback schedules runs after the window closed, and is dropped: a reviewed
fresh-stack scheduler (`queueMicrotask`, `setTimeout`, `.then`, resolved through
its signature) or an `await` before it in an async layer (ownership case
`…/own-setter-deferred-write/001`). A function written elsewhere is in the window
when every reference to it is a call inside one (ownership case
`…/own-setter-outside-closure/001`); one call outside every window drops the
write on that run, and a reference that hands it on, or a callback given to a
function this does not know, leaves it uncertifiable. Fixture
`store-setter-extent`.

1.x setters never unlocked the proxy; 1.x is not analyzed here (ADR 0110).

Not reported, deliberately:

- `delete` of a required or readonly property: tsc reports TS2790 (under
  strictNullChecks) or TS2704. A non-optional property typed `T | undefined`
  stays silent too (TS2790 under `exactOptionalPropertyTypes`). Only a property
  the checker marks optional and not readonly (`v?: T`, `Partial<T>`) is
  reported, through the Type Fact `writableOptionalProperty`. A type
  assertion on the operand (`delete store.v!`, `delete (store.v as T)`) is
  TS2703, and a link that may be `undefined` or `null` (`store.o.b` with `o?:`)
  is TS18048/TS18047: both silent. Any other chain past the root
  (`store.a.b`) is uncertifiable: without `strictNullChecks` a link's type
  proves nothing at run time. An element access, an optional chain and a
  cast root are refused; an exact `const` alias of the store is the store
  (ADR 0463), for an assignment and a `delete` alike. Pinned in the tsc oracle and ownership cases
  `…/delete-required-property/001`, `…/delete-optional-property/001`; fixture
  `store-delete`.
- Reassigning an accessor binding (`count = 2`, this rule's former
  accessor-binding arm). On rc.17 a `const` binding is TS2588 and a `let`
  assigned a number or a function is TS2322 (`count++` TS2356, `count += 1`
  TS2365), so it is TypeScript's: tsc-oracle `removed-because-redundant`.
  What type-checks (`count = otherAccessor`, a `let` store assigned an object
  of its type) is a deliberate rebinding that drops no write (fixture
  `accessor-rebinding`).

- A write to a property of an accessor function (`memo.a = 1`, `count.x++`;
  this rule's former `reactive-value` target). The uncast spelling is
  TypeScript's on rc.17 (TS2339, TS7053 for a computed key under `strict`). A
  cast one type-checks but is no dropped write: an accessor is an ordinary
  extensible function, so the property lands (probed on rc.17: it reads back,
  the accessor is neither frozen nor a proxy), and attaching metadata to a
  function is as often deliberate. tsc-oracle `removed-because-redundant`.

A write to a store **root record's own property** is TypeScript's on the
releases whose typings make it `readonly`, and this rule's where they do not.
The runtime drops the write outside a setter either way. The difference is the
declared type:

- On `@solidjs/signals` `2.0.0-rc.0` through `rc.6`, `Store<T> = Readonly<T>`,
  so `store.name = "b"` is
  `TS2540: Cannot assign to 'name' because it is a read-only property`.
  This rule stays silent there, because it never reports what TypeScript
  already reports.
- From `rc.7` (`rc.7`-`rc.9`, `rc.13`, `rc.17`), `Store<T> = T`, so the same
  write type-checks and nothing else would report it. This rule reports it
  there. rc.17's dev build warns `STORE_WRITE_OUTSIDE_SETTER` on the same write.

The declaration is `@solidjs/signals`'s, which `solid-js` re-exports, and a
fresh install of `solid-js@2.0.0-rc.3` resolves signals rc.9 today. So the
answer follows the `@solidjs/signals` the installed `solid-js` resolves, not
the `solid-js` version. The rule does not decide this itself: the dialect's
vocabulary for the detected installation answers it
(`rust/crates/solid-dialect/src/solid_2/releases.rs`). On a signals release
nobody compared, or none resolved, the rule stays silent on a root write,
because an older signals would make it TS2540, and the
[unaudited-solid-release](unaudited-solid-release.md) notice says root writes
went unchecked. Nested records, props objects and cast-away roots are writable
to TypeScript on every release, so they stay this rule's everywhere. The
fixtures `fixtures/reactive-ir/store-root-write-rc3`, `store-root-write-rc9`,
`release-triple-store-rc7` and `release-triple-store-fresh-rc3` (`solid-js`
rc.3 over signals rc.9) pin each side against byte-faithful copies of the
published declarations.

Read-modify-write spellings are writes in both dialects: `store.count += 1` and
`store.count++` reach the proxy with a value that is dropped, and both also read
the old value, so [strict-read-untracked](strict-read-untracked.md) reports the
read alongside this write. A plain `=` reads nothing and produces this finding
alone.

The warning tier mirrors eslint-plugin-solid's `reactivity` policy for this
defect and stays consistent across the two dialects. It does not mean the
write is speculative: every finding is a proven mutation that bypasses the
reactive setter.

## Why is this bad?

Solid updates the graph only through setters: the setter is what notifies
subscribers. A direct assignment or in-place mutation changes the underlying
value silently — no computation re-runs, no JSX updates, and later setter-driven
updates may clobber the change. Props are readonly by design: the parent owns
the value.

## Examples

Examples of **incorrect** code for this rule:

```tsx
const [items] = createStore({ list: [3, 1, 2] });
// An in-place array method on a store path: dropped.
items.list.sort();

const [doc] = createStore<{ note?: string }>({ note: "a" });
// The `deleteProperty` trap drops it too.
delete doc.note;

const selected = createProjection((draft) => { draft[id()] = true; }, {});
// Derived, read-only outside its compute function: dropped.
selected.other = true;

const [profile] = createStore({ name: "Ada" });
const [, setOther] = createStore({ n: 0 });
// Another store's setter does not unlock this proxy; the write is dropped.
setOther(() => {
  profile.name = "Grace";
});
```

Examples of **correct** code for this rule:

```tsx
const [user, setUser] = createSignal({ name: "Ada" });
setUser({ ...user(), name: "Grace" });

// A write through a *called* accessor is not this rule's claim: the call hands
// back the value, so nothing is dropped. `el()!.style.color = "red"` is the
// same shape and is how imperative DOM work is written in Solid.

// Or use a store and mutate the draft in its own setter:
const [profile, setProfile] = createStore({ name: "Ada" });
setProfile((draft) => {
  draft.name = "Grace";
});

// Inside the store's own setter the original proxy is write-enabled too:
setProfile(() => {
  profile.name = "Grace"; // commits — the store is in its Writing set
});
```

## How to fix

Route every update through the setter returned by `createSignal` or
`createStore` — for stores, mutate the draft (or the original proxy) inside the
store's own setter callback. For state a child needs to change, lift it to the
parent and pass a callback down instead of assigning to props.

## Related

- [no-write-in-pure-computation](no-write-in-pure-computation.md) — setter calls in the wrong scope
