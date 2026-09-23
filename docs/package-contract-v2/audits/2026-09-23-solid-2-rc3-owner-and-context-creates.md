# Audit: Solid 2.0.0-rc.3 — `runWithOwner`, `createContext` and `useContext` in the `creates` call domain

Date: 2026-09-23. Status: **wired into the negative table the same day; for the
repository owner's review**, as
[the 2026-09-04 core-primitives audit](2026-09-04-solid-2-rc3-core-primitives-creates.md)
was. It follows that audit's method (§ 1.3) and terms (§ 2) exactly, and
decides only what `semantic-model.md` § creates lets it decide: whether one
invocation performs a published `create` — **registering a version-1 resource
into a runtime outside this invocation**, a browser document or a server
runtime. Reactive owners, context maps and computations coming into existence
are not `create` operations, and a callable the caller supplied is excluded.

## 0. Why these three

The generator's `creates` proposal walk declines an export whose body calls a
dialect primitive the table has no row for (`dialect-silent`).
`phase22/2026-09-23-what-holds-an-import-open.md` found these three among the
calls that decline it: `@solid-primitives/rootless`' roots call `runWithOwner`,
and context helpers call `createContext` and `useContext`. `getOwner`,
`onCleanup`, `createRoot` and `untrack` already have rows (2026-09-04 audit §§
3-6); `createSignal`, `createMemo` and `createEffect` through `solid-js` do not,
and must not until the table can state a guarded row (that audit's § 7.4).

## 1. `runWithOwner` — archive `@solidjs/signals@2.0.0-rc.3`

### 1.1 Declaration

`@solidjs/signals/dist/types/core/core.d.ts:141`:
`export declare function runWithOwner<T>(owner: Owner | null, fn: () => T): T;`
— entry `dist/types/index.d.ts:1`; `solid-js/types/index.d.ts:1` re-exports it,
so a `solid-js` import resolves to this declaration and the row is keyed on
this archive (2026-09-04 audit § 1.4).

### 1.2 Implementation, all three bundles

| Bundle | Lines | Body |
| --- | --- | --- |
| `dist/prod/core/core.js` | 859-870 | saves `context` and `tracking`, sets `context = e`, `tracking = false`, `return t()` in `try`, restores both in `finally` |
| `dist/node.cjs` | 3140-3151 | byte-identical to prod |
| `dist/dev.js` | 4232-4256 | the same body, preceded by one dev-only branch: when `owner && owner._flags & REACTIVE_DISPOSED`, `emitDiagnostic({...})` (`:748-756`, installed diagnostic listeners) and `console.warn(message)` (**host**, console only) |

`context` and `tracking` are module-level `let`s (`core.js:21`, `:41`;
`dev.js:3174`, `:3190`).

| Callee | Reach | Disposition | What it does |
| --- | --- | --- | --- |
| `t()` / `fn()` | always | parameter-rooted | the caller's callable; whatever it registers is its own, excluded by § creates |
| `emitDiagnostic(...)` | dev, disposed owner only | local | pushes an entry to installed listeners and captures; registers nothing with a host |
| `console.warn(...)` | dev, disposed owner only | host | writes to the console; no resource |

**3 rows**, none a registration. Swapping the ambient owner is a reactive-graph
state change, not a `create` (2026-09-04 audit § 3.2 reads `createRoot`'s owner
the same way).

### 1.3 The `solid-js` server condition

The row binds the declaration's archive, and a `solid-js` import under the
`node` condition runs `solid-js`' own body instead, so a row is sound only
when both were read (2026-09-04 audit § 1.4). `solid-js/dist/server.js:82-90`
(`server.cjs:83-91`, byte-identical): saves `currentOwner` (`let`, `:18`), sets
it, `return fn()` in `try`, restores in `finally`. **Same verdict.**

### 1.4 Verdict

**`creates` closed** for `(@solidjs/signals, runWithOwner)`, in every bundle
and condition. Not offered for other domains: `callbacks` is positive by
construction (it invokes `fn`), and the dev diagnostic invokes installed
listeners.

## 2. `createContext` — archive `solid-js@2.0.0-rc.3`

### 2.1 Declaration

`solid-js/types/client/core.d.ts:84`:
`export declare function createContext<T>(defaultValue?: T, options?: EffectOptions): Context<T>;`
— re-exported by `types/index.d.ts:3` from `./client/core.js`. This is
`solid-js`' own definition, not `@solidjs/signals`' (whose `createContext`,
`prod/core/context.js:12`, `solid-js` does not re-export), so the row is keyed
on `solid-js`.

### 2.2 Implementation, all six bundles

| Bundle | Lines | Note |
| --- | --- | --- |
| `dist/solid.js` | 6-17 | browser `import` |
| `dist/dev.js` | 5-16 | browser `development` `import`, byte-identical to `solid.js`'s |
| `dist/server.js` | 1427-1438 | `node`/`worker`/`deno` `import`, byte-identical |
| `dist/solid.cjs` | 7-18 | the same body calling `signals.createRoot` and `signals.setContext` |
| `dist/dev.cjs` | 6-17 | byte-identical to `solid.cjs`'s |
| `dist/server.cjs` | 1428-1439 | byte-identical to `server.js`'s |

```js
function createContext(defaultValue, options) {
  const id = Symbol(options && options.name || "");
  function provider(props) {
    return createRoot(() => {
      setContext(provider, props.value);
      return children(() => props.children);
    });
  }
  provider.id = id;
  provider.defaultValue = defaultValue;
  return provider;
}
```

| Callee | Reach | Disposition | What it does |
| --- | --- | --- | --- |
| `Symbol(...)` | always | standard library | a fresh symbol; no registration |

**1 row.** `provider` is a function *defined*, not called: its body — a root,
a context write, a `children` memo — runs when a consumer renders the returned
provider, which is that consumer's invocation of a value this call handed back,
not an operation "because of this call". The two property writes are on that
fresh function object.

### 2.3 Verdict

**`creates` closed** for `(solid-js, createContext)`, in every bundle and
condition.

## 3. `useContext` — archive `solid-js@2.0.0-rc.3`

### 3.1 Declaration

`solid-js/types/client/core.d.ts:116`:
`export declare function useContext<T>(context: Context<T>): T;` — `solid-js`'
own, as for `createContext`.

### 3.2 Implementation, browser and development bundles

`dist/solid.js:18-20`, `dist/dev.js:17-19`: `return getContext(context);`,
importing `getContext` from `@solidjs/signals`. `dist/solid.cjs:19-21` and
`dist/dev.cjs:18-20` call `signals.getContext(context)`.

`@solidjs/signals`' `getContext` (`prod/core/context.js:28-37`,
`dev.js:4454-4463`, `node.cjs:3331-3340`): `throw new NoOwnerError` without an
owner; otherwise reads the owner's context map through `hasContext`
(`context.js:59-61`) or falls back to `context.defaultValue`, and
`throw new ContextNotFoundError` when that is `undefined`. Both error classes
are local `Error` subclasses whose constructors call `super(message)`.

| Callee | Reach | Disposition | What it does |
| --- | --- | --- | --- |
| `getContext(context)` | always | dialect, `@solidjs/signals` | reads an owner's context map; may construct and throw a local `Error` |

### 3.3 The `solid-js` server condition

`dist/server.js:1439-1449` (`server.cjs:1440-1450`): calls its own
`getContext` (`server.js:103-110`, the same read over `currentOwner._context`)
in a `try`; on a `ContextNotFoundError` it asks `serverComponentContextError`
(`:1382-1387`, which reads `currentOwner._context` and may construct an
`Error`) and throws that, and otherwise rethrows. **Same verdict.**

### 3.4 Verdict

**`creates` closed** for `(solid-js, useContext)`, in every bundle and
condition: it reads a context map and at most constructs an error. `throws` is
positive by construction and not a version-1 census target.

## 4. What this changes, and what it does not

Three rows join the negative table, each citing its definition in every bundle
the table's archive ships (`rust/crates/solid-dialect/src/solid_2.rs`), with the
cited bytes checked in under `rust/crates/solid-dialect/audited-slices/`. A call
to one of them no longer declines a caller's `creates` proposal as
`dialect-silent`; the caller's census still has to clear everything else it
calls. They change nothing for a call that resolves an archive other than rc.3:
the rows are archive-keyed, exactly as the 2026-09-04 rows are.
