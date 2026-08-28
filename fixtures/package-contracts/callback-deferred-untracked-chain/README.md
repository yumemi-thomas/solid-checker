# Callback wrapper chains preserve schedule and tracking independently

This fixture exercises nested Solid 1.x callback wrappers during temporary-v2
proposal generation. Schedule and tracking are separate operation axes:

- `mountShape`, `mountShapeArrow`, and `mountThroughLocalUntrack` produce a
  queued, untracked callback operation. This matches `onMount`'s
  `createEffect(() => untrack(fn))` shape.
- `memoShape`, `memoThroughLocalUntrack`, `renderEffectShape`,
  `mergePropsShape`, `inlineShape`, and `inlineThroughLocalUntrack` execute
  same-stack; their tracking state follows the innermost clearing wrapper.
- `cleanupShape` remains queued/untracked.
- `memoInsideUntrack` and `trackedShape` preserve tracked behavior even though
  the surrounding lexical context differs.
- the three `unestablished...` exports do not invent an operation when the
  wrapper's schedule is not proven; only those local leaves remain open.

The critical Phase 14 regression is `mountShape`: identity forwarding and the
explicit-arrow twin now normalize to the same queued/untracked operation. The
proposal plan records the operation-local owner-production claims separately,
so uncertainty on ownership cannot contaminate known schedule or tracking.

This is still an unaccepted proposal. Its omitted `closed` leaves are proof
obligations, not schema-v1 sentinels, and runtime probes may only falsify a
candidate. Proof replay is required before any closure can appear in an
accepted document.

The reduced Solid declarations and runtime helper remain faithful to the
published 1.9.14 behavior used by these paths, and the fixture is clean under
`tsc --noEmit`.
