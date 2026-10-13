# write-scope-roots-rc3

Four follow-ups to `setter-in-root-rc3`, each about where a root owner is the
ambient owner of a write or an action call, and so about SC2001
(`reactive-write-in-owned-scope`) and SC2002 (`action-called-in-owned-scope`):

1. **A component body is a root in dev** (`components.tsx`). `solid-js`'s dev
   `createComponent` runs every component as `createRoot(() => untrack(() =>
   Comp(props)), { transparent: true })` (rc.3 `dist/dev.js:35-53`), so a
   write directly in the body meets the same guard a write directly in a
   `createRoot` body does. On rc.1-rc.8 the store setter's guard exempts a root,
   so a store setter directly in a component body is legal there, and SC2001
   used to report it. The dialect answers this with
   `component_body_runs_under_root` plus the release-dependent
   `store_setter_guard_exempts_roots`.
2. **`flush(fn)` keeps the caller's owner** (`roots.ts` § 1). It runs `fn`
   inline without touching the owner or the listener (rc.3
   `dist/dev.js:1788-1802`), so a write in it is as legal as at the `flush`
   call: `createMemo(() => flush(() => setCount(3)))` throws. SC2001 used to
   treat `flush`'s callback as a legal write region.
3. **An action call in a root body** (`roots.ts` § 2) throws
   `ACTION_CALLED_IN_OWNED_SCOPE` on every release; SC2002 had no root-body arm,
   and missed an action call in a memo nested in a root for the same reason.
4. **A function passed to `createRoot` by name** (`roots.ts` § 3) is the root
   body. It is resolved by symbol to a same-file declaration (a `function`
   declaration or a `const` bound to an arrow); a parameter that shadows a
   module function of the same name does not resolve to it.

The twin is `write-scope-roots-rc9`; the two share `roots.ts` and
`components.tsx` byte for byte and differ only in the installed versions and the
declarations copied from them.

**Premise.** Both rules follow the dev client build, as every one of their arms
does; the prod builds carry no guard and nothing below throws there. Probed with
`solid-js` and `@solidjs/signals` from the published tarballs, each at one
release, rc.0-rc.9, dev client builds:

| case | rc.0 | rc.1-rc.8 | rc.9 |
| --- | --- | --- | --- |
| signal or optimistic-signal setter directly in a component body (also via `untrack`, `flush(fn)`, a nested component) | throws | throws | throws |
| `createStore` setter directly in a component body (also via `untrack`, `flush(fn)`, a helper, a nested component) | throws | legal | throws |
| `createOptimisticStore` setter directly in a component body | legal | legal | throws |
| a store setter in a memo compute in a component body | throws | throws | throws |
| a signal setter in `flush(fn)` in a memo compute, a component body or a root body, inline or by name | throws | throws | throws |
| the same `flush(fn)` at module scope | legal | legal | legal |
| an action call directly in a root body, the dispose form, inside `untrack`, or in a memo nested in a root | throws | throws | throws |
| an action call in an effect apply or `onSettled` inside a root, or at module scope | legal | legal | legal |
| a function passed to `createRoot` by name: signal setter, action call | throws | throws | throws |
| the same: `createStore` setter / `createOptimisticStore` setter | throws / legal | legal / legal | throws / throws |

rc.0 keeps the store exemption in the dialect (`StoreSetterRootGuard`), because
its two store setters disagree under a root and a write carries only "a store
setter": its `createStore` setter in a component body is therefore a miss, not
a claim.

Expected findings, rc.3:

- `components.tsx`: `setCount(1)` and `setPending(1)` in `Counter`,
  `setCount(2)` in `Row`, and the store setter in the memo compute inside
  `Counter`: **SC2001**, violation. Every store setter directly in a component
  body, through `untrack`, `flush(fn)` or `writeStoreFromCounter`: **none**.
- `roots.ts`: `setCount(2)` in `flushWrite` (passed to `flush` in a memo),
  `setCount(3)` and `setCount(4)` in `flush(fn)` in a memo compute and a root
  body, `setCount(7)` in `init`, `setCount(8)` in `initWithDispose`:
  **SC2001**. `save()` directly in a root body, inside `untrack` in the dispose
  form, in a memo nested in a root, and in `init`: **SC2002**. The module-scope
  `flush` calls, the event listeners, the effect apply, `onSettled`, the store
  setter in `init`, and `notARootBody`: **none**.

**Stubs.** `node_modules/@solidjs/signals/index.d.ts` copies every declaration
the cases use byte for byte from the published `@solidjs/signals@2.0.0-rc.3`
`dist/types/`, dropping only the JSDoc block before each; its header names the
source files. `node_modules/solid-js/index.d.ts` re-exports them as
`types/index.d.ts` does, copies `createOptimistic`, `createOptimisticStore`
and the three local types they use byte for byte from
`types/client/hydration.d.ts`, and says where it reduces the other hydration
consts; `node_modules/solid-js/types.d.ts` is `types/types.d.ts` whole.
`node_modules/@solidjs/web/jsx-runtime.d.ts` is reduced to the `JSX.Element`
and children-attribute lines of `types/jsx.d.ts`, byte for byte: no case
renders an intrinsic element and no rule reads JSX typing. Both sources are
`tsc --noEmit` clean (TypeScript 5.9.3, `strict`, bundler resolution,
`jsxImportSource: "@solidjs/web"`) against these stubs and against the real
rc.3 install, and the checker reports the same findings against both.
