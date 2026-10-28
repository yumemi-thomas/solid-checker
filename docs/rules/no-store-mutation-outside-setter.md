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
- a call of one of the nine in-place `Array.prototype` methods (`push`, `pop`,
  `shift`, `unshift`, `splice`, `sort`, `reverse`, `fill`, `copyWithin`) on a
  path of the store. The callee must resolve through Type Facts to the
  standard library's `Array` method, so a project method of the same name, a
  typed array's `fill` and an unresolved receiver are silent (upstream's
  `no-store-mutation-outside-setter` matches the method name);
- the same writes through a `createProjection` result, reported as a
  projection write (it has no setter to name).

**Past the root (ADR 0465).** Only a write that reads no link past the root
(`store.v = 1`, `list.push(1)` on a store that is the array, a cast root) is a
violation. A write through a link (`store.a.b = 1`, `store.items.push(1)`,
`store.rows[0].done = true`, `store.n!.b = 1`) is silent where the link's type
may hold `null` or `undefined` (TS18048/TS18047 under strictNullChecks: the
write is TypeScript's), and uncertifiable otherwise: the facts do not record
strictNullChecks, so a link's type proves nothing about its runtime value, and
an undefined link throws `TypeError` before the proxy sees a write. Fixtures
`store-write-chain-links`, `store-write-chain-links-loose`.

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
after `flush()`):

- A write through the original store proxy **lexically inside one of that
  store's own setter callbacks**: `storeSetter` adds the store to
  `writeScopes` for the callback (`dist/dev.js:5847-5866`), so the trap's
  `inDraft(target)` holds (`:4955-4957`). `setStore(draft => { store.value = 7
  })` commits like a draft write. The exemption is proven, not guessed: the
  callee must resolve to the setter destructured beside that exact store.
  Writing through *another* store's proxy inside a setter is dropped (a
  different `scopeKey`) and stays a finding, as does any write outside a
  setter. Upstream reports the own-setter write; that divergence is pinned as
  ownership case `no-store-mutation-outside-setter/own-setter-write/001`.
- A write through a projection's result symbol (or a function-form
  `createStore(fn, seed)`'s store) **lexically inside its compute function**:
  `runProjectionComputed` runs the function inside `storeSetter(draft, …)`
  (`dist/dev.js:6675-6678`). Ownership case `…/projection-compute-write/001`.

1.x setters never unlocked the proxy; 1.x is not analyzed here (ADR 0110).

Not reported, deliberately:

- `delete store.path`. It is dropped, but for a required property tsc reports
  TS2790 (strictNullChecks), and a property whose type includes `undefined` or
  is optional type-checks with no Type Fact at a delete operand to say which.
  Pinned in the tsc oracle and ownership case `…/delete-required-property/001`;
  open item in `docs/open-work.md`.
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
