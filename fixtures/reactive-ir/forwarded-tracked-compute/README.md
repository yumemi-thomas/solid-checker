# forwarded-tracked-compute

**Claim (ADR 0200).** Consider a function literal passed to a project function
whose only use of that parameter is a direct call written in the tracked
compute of a computation it creates. That literal runs only inside that
tracked compute, so a read written directly in it is tracked, not an
untracked read in the caller's body
(`execution_role::forwarded_tracked_compute_role`).

The compute must be the whole argument, written in the wrapper's own body, at
a slot that tracks its callback's reads:

- a primitive's tracked callback (`createMemo`'s compute, here); or
- an accepted contract's guaranteed tracked-compute slot (ADR 0183; the rc.13
  corpus's `useQuery` wrappers).

| Case | Finding | Why |
| --- | --- | --- |
| `UsesTracked` | none | `tracked` invokes `read` only in `createMemo(() => read())` |
| `UsesTrackedAndEager` | `SC1001` violation | the wrapper also calls `read()` in its body, during the call |
| `UsesUntracked` | `SC1001` uncertifiable | the call sits in an `untrack` callback, which does not track |
| `UsesTrackedLater` | `SC1001` uncertifiable | an async wrapper |
| `UsesForwarded` | `SC1001` uncertifiable | the parameter is handed on uncalled (`createMemo(read)`), not invoked in a literal |

The stubs are copied from `forwarded-event-prop`.
