# `onSettled` in a call whose callee is `any` (ADR 0867)

`(onSettled as any)(cb)` type-checks and runs: with no owner, rc.21 throws
SETTLED_CLEANUP_UNOWNED once the callback returns a cleanup. It is proven here, and only
a call tsc rejects may be dropped when the callback's return is unproven. The typed call
whose callback returns nothing has no cleanup to report.
