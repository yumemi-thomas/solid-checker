# store-root-write-rc9

On `solid-js@2.0.0-rc.9` a write to a `createStore` root's own property type-checks (`@solidjs/signals@2.0.0-rc.9` declares `Store<T> = T`, `dist/types/store/store.d.ts:4`; real rc.9 `tsc --noEmit` and this fixture's byte-faithful stub report nothing) and the runtime drops it (dev and prod), so SC2003 reports `profile.name = "Grace"` (line 9, vocabulary `store_root_properties_are_readonly() == false`) and `profile.user.name` (line 12); writes inside `setProfile(...)` are none; SC9014 at the `solid-js` manifest (rc.9 is reviewed with gaps). Twin `store-root-write-rc3`.
