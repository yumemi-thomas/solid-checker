# SC5001 and SC1001: a hook's memo, `untrack` and labelled windows

`const m = useM()`, with the hook returning the creation itself, a bound memo or an
object slot, creates the memo under the body's owner: a plain read throws
`PENDING_ASYNC_UNTRACKED_READ` (it was SC1001 for a bare `return createMemo(...)`), and
so does an unlabelled `untrack`, a reference `untrack(m)`, and the layers that run on
the untrack's stack (ADR 0628). A module-scope instance suspends, a root's is not
followed, and two creations in one hook stay open. A label opens a window anywhere;
`false` opens none. Measured on rc.21 in Chrome.
