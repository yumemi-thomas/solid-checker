# store-root-write-rc3

The negative twin of `store-root-write-rc9`: on `solid-js@2.0.0-rc.3` a write
to a `createStore` root's own property is TypeScript's, so SC2003 stays silent
on it.

`@solidjs/signals@2.0.0-rc.3` declares `Store<T> = Readonly<T>`
(`dist/types/store/store.d.ts:4`). `tsc --noEmit` (5.9.3, `strict`, bundler
resolution) over `store.ts` against the **real published** `solid-js@2.0.0-rc.3`
install, not this stub, reports:

```
store.ts(9,11): error TS2540: Cannot assign to 'name' because it is a read-only property.
store.ts(21,13): error TS2540: Cannot assign to 'name' because it is a read-only property.
```

The same two errors come from this fixture's stub, because every declaration it
carries is byte-faithful to the published one (see
`node_modules/solid-js/index.d.ts` for what was reduced).

Expected findings:

- `profile.name = "Grace"` outside a setter (line 9): **none**. It is TS2540,
  and the checker never reports what TypeScript already reports. The dialect's
  rc.3 vocabulary answers `store_root_properties_are_readonly() == true`.
- `profile.user.name = "Grace"` (line 12): **SC2003**. The readonly-ness is
  shallow, so TypeScript accepts it, and the runtime drops it.
- Both writes inside `setProfile(...)`: **none**. The store's own setter
  write-enables its proxy (TS2540 still fires on line 21, which is the type's
  claim, not a defect this rule asserts).
- No `SC9014`: rc.3 is the audited release.
