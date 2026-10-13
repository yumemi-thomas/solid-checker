# setter-in-root-rc9

The rc.9 twin of `setter-in-root-rc3` (read that README for the probe table
and the premise): the same `root.ts`, with `solid-js`, `@solidjs/signals` and
`@solidjs/web` at `2.0.0-rc.9`.

`@solidjs/signals@2.0.0-rc.9` removed the root exemption from
`devGuardStoreSetterWrite`: `if (context && !(context._config &
CONFIG_CHILDREN_FORBIDDEN))` (`dist/dev-shared.js:5878`), whose comment cites
#3500, "Roots are NOT exempt". Probed on the published rc.9 dev client build, a
store setter directly in a `createRoot` body throws
`REACTIVE_WRITE_IN_OWNED_SCOPE`; on rc.1-rc.8 the same call is legal. The
dialect answers this from the resolved signals
(`store_setter_guard_exempts_roots`, `StoreSetterRootGuard` in
`rust/crates/solid-dialect/src/solid_2/releases.rs`), so this triple's
vocabulary reports it.

Expected findings, beyond the twin's:

- Every store setter directly in a root body, or inside `untrack` there, or in
  the helper only a root body calls (lines 24, 36, 46, 55, 70, 79): **SC2001**,
  violation.
- **SC9014** `unaudited-solid-release` at `node_modules/solid-js/package.json`:
  the rc.9 triple is reviewed with gaps still open.

Everything the twin reports is reported here too, and everything it leaves
silent other than the store setters above stays silent.

**Stubs.** As in the twin, from the published rc.9 typings. `solid-js`
declares its own `createRoot` on rc.9, `export declare const createRoot:
typeof coreRoot` in `types/client/hydration.d.ts`, and the stub keeps that
shape, so the case also pins that a `createRoot` const typed from the signals
function is recognized. `root.ts` is `tsc --noEmit` clean against these stubs
and against the real rc.9 install.
