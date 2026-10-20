# fp-owner-leaf-helper-after-await

A helper called synchronously from `onSettled` is credited with the `onCleanup` calls in its *synchronous extent* only: past the first `await` the call runs from a promise continuation with no owner (`NO_OWNER_CLEANUP`, `missing-owner`'s claim), not `CLEANUP_IN_FORBIDDEN_SCOPE`, so no `leaf-owner-forbidden-call` there while an `onCleanup` before the first `await` is one. Stub as `fp-owner-show-children-callback`.
