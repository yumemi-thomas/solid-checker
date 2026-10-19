# release-triple-store-rc7

The rc.7 triple takes rc.9's store answer: `@solidjs/signals@2.0.0-rc.7` declares `Store<T> = T` (`dist/types/store/store.d.ts:4`; stubs byte-faithful to rc.7), so a root property write type-checks and the runtime drops it: SC2003 at lines 9 and 12, none inside `setProfile(...)`, plus SC9014. `store.ts` is `store-root-write-rc9`'s.
