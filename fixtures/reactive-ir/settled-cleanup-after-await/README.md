# settled-cleanup-after-await

**Claim (ADR 0488).** `onSettled` called after an `await` its async function reaches on every path runs on a continuation with no owner: a callback that returns a cleanup throws `SETTLED_CLEANUP_UNOWNED` (`missing-owner`, SC4001), as anywhere unowned. Not proven (uncertifiable, unchanged): the call before the `await`. Clean: a callback that returns nothing. Stub as in `owner-leaf-nested-cleanup`.
