# SC5001: an unlabelled untrack of the body's own pending source

rc.21's `untrack` sets `strictPending` in an active strict-read window, and
`read()` throws `PENDING_ASYNC_UNTRACKED_READ` when `strictPending &&
owner._parent === context` (`@solidjs/signals`, #4630). So a pending memo, derived
store or projection the component body created throws through `untrack`, nested
ones too; a source made elsewhere, or a read under a child root, does not (ADR 0619).
