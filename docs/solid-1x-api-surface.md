# Solid 1.x API surface

The authoritative list of Solid 1.x exports the checker may recognize. Every
name here was extracted from the published package, not from memory or from
documentation.

- Source: `npm pack solid-js@1` → **solid-js 1.9.14**
- Extracted: 2026-07-25 from `types/`, `store/types/`, `web/types/`
- Purpose: the source `rust/crates/solid-dialect/src/solid_1x.rs` cites for its
  vocabulary, the reference for every fixture `solid-js.d.ts` stub, and the
  source of rule documentation examples

The export index is generated from the installed package, not read from this
file, and the two are cross-checked: dialect tests assert every recognized name
is a real value export of an owned module and that its callback timing is
compatible with the receipt-issued exact artifact case. This file governs what
may enter vocabulary; the accepted bundle governs where and under which
runtime/declaration selection it exists.

**Do not add a name to the checker's vocabulary that is not on this list.** If
something seems missing, re-extract from the package and update this file in the
same change.

## Entry points

1.x ships four user-facing subpaths. This matters because the checker's module
gate must admit all of them:

| Subpath | Holds |
| --- | --- |
| `solid-js` | reactivity, control flow, component helpers |
| `solid-js/store` | stores and mutables |
| `solid-js/web` | DOM rendering, `Dynamic`, `Portal`, SSR |
| `solid-js/universal` | custom renderers |

Plus `solid-js/h`, `solid-js/html`, `solid-js/jsx-runtime`,
`solid-js/jsx-dev-runtime`.

> **2.0 contrast, for reviewers:** 2.0 moves store APIs *into* core `solid-js`,
> moves `solid-js/web` to `@solidjs/web`, and sets `jsxImportSource` to
> `@solidjs/web`. In 1.x, `createStore` imported from `solid-js` is wrong, and
> `solid-js/store` is correct.

**Subpath precision.** The Solid 1 bundle index under
`pkg/contracts/bundled/solid-v1/` names separate receipt-issued temporary-v2
documents for root, store, web, universal, `h/jsx-runtime`, and
`h/jsx-dev-runtime` artifact cases across their exact browser/node and
development/production selections. Exports appear only in artifact cases whose
runtime and declarations both provide the binding. `solid-js/jsx-runtime` and
`solid-js/jsx-dev-runtime` have no common runtime/declaration value bindings in
the checked package and therefore stay in the authority census without an
accepted semantic case.

Wrong-subpath imports remain TypeScript's responsibility: SC8002 (`v1/imports`)
was removed on 2026-08-17 because its condition duplicated TS2305. The dialect
export index still provides vocabulary/module ownership, while contract
selection independently requires an exact installed artifact match and never
uses a name-only fallback.

## `solid-js`

### Reactive primitives

`createSignal` · `createMemo` · `createEffect` · `createRenderEffect` ·
`createComputed` · `createReaction` · `createDeferred` · `createSelector` ·
`createResource` · `createRoot`

### Tracking and scheduling

`untrack` · `batch` · `on` · `startTransition` · `useTransition`

### Lifecycle, ownership, errors

`onMount` · `onCleanup` · `onError` · `catchError` · `getOwner` ·
`runWithOwner` · `resetErrorBoundaries`

### Context

`createContext` · `useContext`

### Arrays and children

`mapArray` · `indexArray` · `children`

### Component and props helpers

`mergeProps` · `splitProps` · `lazy` · `createUniqueId` · `createComponent`

### Control flow components

`For` · `Index` · `Show` · `Switch` · `Match` · `Suspense` · `SuspenseList` ·
`ErrorBoundary`

### Interop

`observable` · `from`

### Signatures the checker depends on

```ts
// Two overloads. Callback at index 0; index 1 is a seed VALUE, not a callback;
// index 2 is options.
function createEffect<Next>(fn: EffectFunction<undefined | NoInfer<Next>, Next>): void;
function createEffect<Next, Init = Next>(
  fn: EffectFunction<Init | Next, Next>,
  value: Init,
  options?: EffectOptions & { render?: boolean },
): void;

function createMemo<T>(fn: (v: T) => T, value?: T, options?: MemoOptions<T>): Accessor<T>;

interface BaseOptions   { name?: string }
interface EffectOptions extends BaseOptions {}
interface MemoOptions<T> extends EffectOptions { equals?: false | ((prev: T, next: T) => boolean) }
interface SignalOptions<T> extends MemoOptions<T> { internal?: boolean }
```

`SignalOptions` is the complete set of `createSignal` options in 1.x: `name`,
`equals`, `internal`. **There is no `ownedWrite`.**

## `solid-js/store`

`createStore` · `produce` · `reconcile` · `unwrap` · `createMutable` ·
`modifyMutable` · `isWrappable`

## `solid-js/web`

Rendering: `render` · `hydrate` · `template` · `insert` · `spread` · `effect` ·
`memo` · `use` · `createComponent` · `createDynamic`

Components: `Dynamic` · `Portal` · `Assets` · `HydrationScript` · `Hydration` ·
`NoHydration`

DOM: `classList` · `className` · `style` · `setAttribute` · `setBoolAttribute` ·
`setProperty` · `setStyleProperty` · `addEventListener` · `delegateEvents` ·
`clearDelegatedEvents` · `dynamicProperty` · `assign`

SSR: `renderToString` · `renderToStringAsync` · `renderToStream` ·
`generateHydrationScript` · `getRequestEvent` · `ssr` and the `ssr*` helpers

Note `classList` exists in 1.x and is removed in 2.0 — consistent with the
checker keeping `SC8013 prefer-classlist`.

## Names that do NOT exist in Solid 1.x

Verified by grepping the whole 1.9.14 package: **0 occurrences** of each.

`ownedWrite` · `onSettled` · `createTrackedEffect` · `createProjection` ·
`createOptimistic` · `createOptimisticStore` · `flush` · `affects` · `refresh` ·
`isPending` · `Repeat` · `Loading` · `Reveal` · `action` · `createOwner` ·
`merge` · `omit` · `deep` · `snapshot` · `storePath` · `dynamic` (lowercase
factory) · `createAsync`

One false positive to know about: the string `Errored` appears once in the
package, as the `"errored"` member of `createResource`'s state union. 1.x has no
`<Errored>` boundary — the 1.x error boundary is `ErrorBoundary`.

## Async semantics: 1.x has no suspending read

This is the single most consequential difference for the checker, and it is easy
to get wrong because 1.x *does* support async — `createResource`, `Suspense`, and
`SuspenseList` all exist. What it lacks is **suspension at the read site.**

Verified in `dist/solid.cjs` (1.9.14):

```js
function read() {
  const c = SuspenseContext && useContext(SuspenseContext),
        v = value(), err = error();
  if (err !== undefined && !pr) throw err;   // throws ONLY on fetcher error
  if (Listener && !Listener.user && c) {     // participates ONLY if a SuspenseContext exists
    createComputed(() => { /* c.increment() */ });
  }
  return v;                                   // otherwise returns the value
}
```

and in the resource state union:

```ts
interface Unresolved { state: "unresolved"; loading: false; (): undefined }
interface Pending    { state: "pending";    loading: true;  (): undefined }
```

Consequences:

1. **A pending read returns `undefined`.** It is a total, typed operation, not an
   error and not a suspension.
2. **Reading async data with no boundary is legal and silent.** The Suspense
   branch is skipped outright when no `SuspenseContext` is present. There is no
   1.x analogue of 2.0's `ASYNC_OUTSIDE_LOADING_BOUNDARY`; callers check
   `resource.loading` themselves.
3. **The fetcher runs untracked.** `createResource` invokes it inside
   `untrack(() => fetcher(lookup, { value: value(), refetching }))`, so reactive
   reads inside a fetcher never track — before or after an `await`. The only
   reactive dependency is the `source` argument.

Point 3 is why `reactive-read-after-await` does **not** apply to resource
fetchers. That rule's real 1.x subject is an async callback in a genuinely
tracked computation — `createMemo(async () => { await x; return count() })`,
or the same inside `createEffect` / `createComputed` / `createRenderEffect`.
There, the synchronous prefix tracks and everything after the first `await` does
not, so the computation never re-runs. Reads inside a fetcher are
`strict-read-untracked`, a different rule.

## 2.0 → 1.x mapping

For retargeting 2.0-shaped code and fixtures. "None" means the concept does not
exist in 1.x and the code must be restructured, not renamed.

| 2.0 | 1.x |
| --- | --- |
| `createEffect(compute, apply)` | `createEffect(fn, value?)` — one callback |
| `onSettled` | `onMount` (+ `onCleanup` for teardown) |
| `createTrackedEffect` | None |
| `createProjection` | None (`createMemo` over a store, by hand) |
| `createOptimistic` / `createOptimisticStore` | None |
| `flush` | None — `batch` is 1.x's batching, and it is not a synchronous flush |
| `action` | None in core (`@solidjs/router` has one) |
| `refresh(x)` | `createResource`'s `refetch` |
| `affects` | None |
| `isPending(() => e)` | `resource.loading` |
| `<Loading>` | `<Suspense>` |
| `<Errored>` | `<ErrorBoundary>` |
| `<Reveal>` | `<SuspenseList>` |
| `<For keyed={false}>` | `<Index>` |
| `<Repeat>` | None — no fixed-count component in 1.x |
| `merge` / `omit` | `mergeProps` / `splitProps` |
| `snapshot` | `unwrap` |
| `deep` | None |
| `createOwner` | `getOwner` / `runWithOwner` |
| `dynamic(source)` | `createDynamic` / `<Dynamic component={…}>` from `solid-js/web` |
| `createStore` from `solid-js` | `createStore` from `solid-js/store` |
| `class={{…}}` | `classList={{…}}` |
| Context as provider `<Ctx value>` | `<Ctx.Provider value>` |
| batched-by-default writes | synchronous writes; `batch` to group |
| `createRoot` owned by parent | `createRoot` is detached and must be disposed |
