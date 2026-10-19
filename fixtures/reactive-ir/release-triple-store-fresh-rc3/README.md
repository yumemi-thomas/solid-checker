# release-triple-store-fresh-rc3

A fresh `npm install solid-js@2.0.0-rc.3 @solidjs/web@2.0.0-rc.3` resolves `@solidjs/signals@2.0.0-rc.9` (`^2.0.0-rc.N` ranges), so `Store<T> = T` (byte-faithful to rc.9 `dist/types/store/store.d.ts`) and root writes type-check and are dropped: SC2003 at lines 9 and 12, none inside `setProfile(...)`, plus SC9014 (three packages are not one reviewed release). `tsc` clean against stubs and the real fresh install; `store.ts` is `store-root-write-rc9`'s.
