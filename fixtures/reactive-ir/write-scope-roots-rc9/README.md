# write-scope-roots-rc9

The rc.9 twin of `write-scope-roots-rc3` (read that README for the four claims,
the probe table and the premise): the same `roots.ts` and `components.tsx`,
with `solid-js`, `@solidjs/signals` and `@solidjs/web` at `2.0.0-rc.9`.

`@solidjs/signals@2.0.0-rc.9` removed the root exemption from
`devGuardStoreSetterWrite` (`dist/dev-shared.js:5878`, citing #3500), and the
dev component body is still a root (`solid-js` `dist/solid.dev.js:35-58`,
`observedComponent`). So on this triple every store setter directly in a
component body, or in a function passed to `createRoot` by name, throws
`REACTIVE_WRITE_IN_OWNED_SCOPE`, `createStore`'s and `createOptimisticStore`'s
alike (probed).

Expected findings, rc.9: everything rc.3 reports, plus **SC2001** on

- `components.tsx`: the store setter in `writeStoreFromCounter` (called only
  from `Counter`'s body), `setState` and `setDraft` directly in `Counter`, the
  `setState` inside `untrack` and the `setDraft` inside `flush(fn)` there, and
  `setState` in `Row`;
- `roots.ts`: the store setter in `init`.

The triple also carries the release notice every rc.9 fixture does (SC9014).

**Stubs.** As in the rc.3 twin, extracted from the rc.9 tarballs: rc.9's
`solid-js` declares its own `createRoot` const (`types/client/hydration.d.ts`,
byte-faithful here), and its `createOptimisticStore` const has rc.9's
overloads. `tsc --noEmit` clean against these stubs and against the real rc.9
install, with the same checker findings against both.
