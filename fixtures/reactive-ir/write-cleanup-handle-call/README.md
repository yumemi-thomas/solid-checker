# write-cleanup-handle-call

**Claim (ADR 0456).** rc.17's `onCleanup(fn)` returns `fn` (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:2377-2405`, `dist/dev-shared.js:1415-1421`). Calling the handle runs the callback there, under the caller's owner, with no disposal around it. A write it runs is judged at each handle call too. The handle is found by exact resolution. It is either a `const` binding of the call's result whose every reference is a call in the same file (not exported, no `eval`), or the call written on the result, `onCleanup(fn)()`.

Violations (rc.17 throws at the handle call): the handle called in a memo (`HandleInMemo`), in an effect compute, in a `createRoot` body, at once (`HandleCalledAtOnce`), and in a component body. Also an action call (`ActionHandleInMemo`, SC2002), and a named closure the callback calls (`HandleNamedClosure`). A write under an owner the callback gives it (`runWithOwner(getOwner(), …)`) throws when the handle is called outside a disposal, in the body or in a handler (`OwnerHandle*`).

Legal (rc.17 never throws): a handle nothing calls (`HandleNeverCalled`, `OwnerHandleNeverCalled`; only the disposal runs it, and the guard stands down there), a handle another cleanup calls, a handle called from a handler, and one called from a `setTimeout`.

Uncertifiable: a handle stored, passed, returned, or aliased (`HandleStored`, `HandlePassed`, `HandleReturned`, `HandleAliased`), and a write in an object member the callback builds (`HandleObjectMember`). Checked on the rc.17 dev build; stub as `write-owner-in-cleanup`'s. On a release nobody read for `onCleanup`'s return, a used handle is uncertifiable.

By name: exact is a violation (`HandleByNameInMemo`); a `let` or conditional argument is uncertifiable. A handle call in an unrendered or opaque child or an instance field is uncertifiable; a rendered child is a violation. A write after an await is legal where proven, uncertifiable inside JSX.

Reached: a handle call in a dead branch (`if (false)`), after a `return`, or in code nothing runs is uncertifiable; `if (true)`, a runtime condition and a called closure are violations.
