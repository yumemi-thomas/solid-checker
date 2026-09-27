# setter-in-root-rc3

N3 of the rc.9 vocabulary review
(`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`
§ 5): a setter called directly in a `createRoot` body runs with the root as the
ambient owner, a children-capable owner, and Solid's dev write guard throws
`REACTIVE_WRITE_IN_OWNED_SCOPE` there. SC2001 reports it. Whether a **store**
setter there throws depends on the resolved `@solidjs/signals`: rc.1-rc.8's
`devGuardStoreSetterWrite` exempts a root (`!context._root`, rc.3
`dist/dev.js:4111`), rc.9's does not. On this triple, rc.3, only the signal
setter and `refresh` are reported.

The twin is `setter-in-root-rc9`; the two share `root.ts` byte for byte and
differ only in the installed versions and the declarations copied from them.

**Premise.** SC2001 follows the dev client build, as every one of its arms
does. The prod builds carry no guard, and every case below runs without
throwing there. Probed with `solid-js`, `@solidjs/signals` and `@solidjs/web`
from the published tarballs, each triple at one release, rc.0-rc.9, dev and
prod client builds:

| case | dev rc.3 | dev rc.9 | prod (both) |
| --- | --- | --- | --- |
| signal setter, `refresh(memo)` directly in a root body | throws | throws | legal |
| store setter (plain or derived `createStore`) directly in a root body | legal | throws | legal |
| the same inside `untrack(...)` in a root body | as the body | as the body | legal |
| the same in a helper only a root body calls | as the body | as the body | legal |
| a root created in an effect apply, a memo compute or a component body | as the body | as the body | legal |
| a setter in an effect apply, `onSettled` or an event listener inside the root | legal | legal | legal |
| a setter in a memo compute inside the root | throws (both setters) | throws | legal |
| the same inside `flush(() => …)` in a root body | as the body | as the body | legal |

A signal setter in a root body throws on all ten releases; `setSignal` never
exempted roots. rc.9 is the first release whose store setter guard drops the
exemption (#3500 in its source).

Expected findings, rc.3:

- `setCount(1)`, `refresh(doubled)` in a module-level root body, `setCount(2)`
  in the `Solid.createRoot((dispose) => …)` body, `setCount(3)` inside
  `untrack` in a root body, `setCount(4)` in `writeBoth` (called only from a
  root body), `setCount(5)` and `setCount(6)` in roots created in an effect
  apply and in a memo compute: **SC2001**, violation.
- `setCount(9)` and the store setter beside it in a memo compute inside a root:
  **SC2001**, violation. Before this arm the whole root body was a region
  where writes were legal, so these were missed too.
- Every store setter directly in a root body (lines 24, 36, 46, 55, 70, 79):
  **none**. The rc.3 guard exempts the root.
- `setInternal(1)` (`ownedWrite: true`), the effect apply, `onSettled`, the
  event listener, and the module-scope writes: **none**.
- `flush(() => setCount(10))`: **SC2001**, violation. `flush(fn)` keeps the
  owner exactly as `untrack` does (`callback_preserves_owner_write_context`),
  so the write answers to the root. `write-scope-roots-rc3` covers `flush` in
  a memo compute and a component body, and the callback passed by name.

**Stubs.** `node_modules/@solidjs/signals/index.d.ts` copies every declaration
the cases use byte for byte from the published `@solidjs/signals@2.0.0-rc.3`
`dist/types/`, dropping only the JSDoc block before each and every other
declaration; the file header names the source files.
`node_modules/solid-js/index.d.ts` re-exports them as `types/index.d.ts`
does, and says where it reduces the hydration consts. `root.ts` is `tsc
--noEmit` clean (TypeScript 5.9.3, `strict`, bundler resolution) against these
stubs and against the real rc.3 install.
