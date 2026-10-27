# write-owner-in-cleanup-rc13

**Claim.** `@solidjs/signals@2.0.0-rc.13` has no `!disposalDepth` test in its write guards (rc.17 added it), so the write an `onCleanup` callback runs under a proven owner throws there: a violation (`CleanupRunWithOwner`, `CleanupCreateRoot`), as is every other shape in `write-owner-in-cleanup-unread`. Stub: `solid-js` and `@solidjs/signals` rc.13.
