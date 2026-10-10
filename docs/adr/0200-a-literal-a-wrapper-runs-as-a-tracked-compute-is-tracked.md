# ADR 0200: A literal a wrapper runs as a tracked compute is tracked

- Status: accepted (2026-10-05). Track A, step A3, of
  `docs/2026-10-05-package-direction.md`.
- Owner: `forwarded_tracked_compute_role` in
  `solid-reactive-ir/src/execution_role.rs`.
- Fixture: `fixtures/reactive-ir/forwarded-tracked-compute`.

## Context

Applications wrap hooks: `ai-memory-ui`'s `useQuery` hands
`() => { const resolved = options(); … }` to the real `useQuery`, and
probus-hk's `observe` runs `createEffect(() => options(), …)`. A literal passed
to such a wrapper (`useQuery(() => ({ enabled: open() }))`) runs inside the
compute the wrapper creates. A read written in it is therefore tracked.

`callee_callback_timing` sees only that the wrapper does not invoke the
parameter directly in its own body, so the read stayed uncertifiable. A
proof that the literal is "invoked during the call" would not be enough
either: the lexical role would then report the read as an untracked read in
the caller's body.

## Decision

A function literal passed, as the whole argument, to a project function takes
the tracked role (`ExecutionRole::TrackedJsx`) for code written directly in it
when:

1. **The wrapper resolves exactly** to one project function, which is neither
   async nor a generator, and whose parameter at that slot is one identifier
   with no default.
2. **Every reference to the parameter** is a direct call `p()` written
   directly in one function literal `compute`.
3. **`compute` is a tracked slot.** It is the whole argument of a call written
   directly in the wrapper's body, at a slot that tracks its callback's reads:
   - a primitive's tracked callback (`createMemo`'s or `createEffect`'s
     compute: `callback_execution_at_call` and `tracks_reads`); or
   - an accepted contract's guaranteed tracked-compute slot (ADR 0183's
     `guaranteed_callback_parameters`: the authored `useQuery`).

A parameter referenced any other way (called in the body, handed on uncalled,
called in an untracked callback, or used by an async wrapper) can run the
literal elsewhere too, so it proves nothing.

## Consequences

- A read in a callback a wrapper only runs as a tracked compute is classified
  as tracked, as if written in that compute.
- Still open:
  - wrappers of wrappers (the wrapper's own argument forwarded through another
    project function);
  - a parameter invoked inside a nested function of `compute`;
  - spans in functions nested inside the literal, which keep their own arms.

## Evidence

- Fixture: one clean case (`UsesTracked`) and four negatives. One negative,
  `UsesTrackedAndEager`, stays a proven violation because the wrapper also
  calls the parameter in its body. No other fixture moves: 169 projects, 898
  findings.
- rc.13 corpus, against the ADR 0199 extension sweep: 40 sites leave `SC1001`
  uncertifiable (6,856 to 6,816 findings), most through `ai-memory-ui`'s
  `useQuery` wrapper and probus-hk's `observe`. No violation moved.
