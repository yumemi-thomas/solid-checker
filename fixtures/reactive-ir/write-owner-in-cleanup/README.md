# write-owner-in-cleanup

**Claim.** rc.17's write guards test `!disposalDepth` (`@solidjs/signals` `dist/dev-shared.js:5620-5626`), which `runDisposal` raises around every `onCleanup` callback (`:1314`). A signal write such a callback runs under a proven owner (`runWithOwner(getOwner(), …)`, `createOwner()`, `createRoot`), directly or in a `forEach`, lands: legal (`Cleanup*Owner*`, `CleanupCreateRoot`). A named function a cleanup hands on or calls is uncertifiable (`CleanupNamed*`, `CleanupCalls*`): its other callers are not read.

Controls (violations, rc.17 throws): an effect apply's returned cleanup (`EffectReturnedCleanup`), a write the cleanup schedules or stores (`CleanupSchedulesWrite`, `CleanupStoresFunction`), a write in the body, an action call (no `disposalDepth` test). Stub: rc.17 shapes.
