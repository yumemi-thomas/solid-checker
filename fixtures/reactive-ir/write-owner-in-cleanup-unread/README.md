# write-owner-in-cleanup-unread

**Claim.** `@solidjs/signals@2.0.0-rc.15` is a release no review read, so whether its write guards stand down in a disposal is unknown. A write an `onCleanup` callback runs under a proven owner is then uncertifiable (`CleanupRunWithOwner`, `CleanupCreateRoot`): neither legal (rc.17 only) nor a violation (rc.13 throws). A write the guard rejects on every release stays a violation: in the body, past an await, in an effect's returned cleanup. Stub: `solid-js` rc.17 over signals rc.15.
