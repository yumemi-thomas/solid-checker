# Invocations in different non-reporting roles (ADR 0323, review round 9)

ADR 0319 answered one role per callback and refused when its invocations ran
in more than one. A query hook's `key` runs in a lazy memo's compute (tracked)
and in `untrack(options.key)` (explicitly untracked); a handler is called from
an event and a tracked computation. No invocation of such a callback is a
reporting role, so the reads in it are never reported, whichever invocation
runs. `function_role` now merges roles that all keep a read out of every
reporting role (tracked, deferred, event) into the no-claim role, which every
read rule suppresses; a reporting role among them still refuses. The walk also
follows a function value handed to a primitive's callback slot
(`untrack(options.key)`, `createMemo(f)`) into that slot's role when the
dialect states one that keeps reads out of the window (never an effect's
apply).

`App.tsx`: `TrackedAndUntrack`, `TrackedAndEvent` are now silent;
`SlotByName` was already silent (ADR 0315). Refusals, still uncertifiable:
`TrackedAndBody` (one invocation in the hook's own body, which runs in the
render window), `TrackedAndApply` (one in an effect's apply), `SlotApply` (a
function handed to an apply slot). Every component is exported so an
unplaced read is an obligation rather than a violation. The tree passes
`tsc --noEmit` against the published rc.13 declarations (copied unchanged from
`standard-runtime-configuration`).
