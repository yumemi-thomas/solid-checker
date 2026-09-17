# Multi-role callback parameters stay locally open

This fixture pins the stable-v1 replacement for the former legacy-v1
unknown sentinel. A parameter reached through incompatible execution sites
does not emit contradictory callback rows and does not close the callback
domain. The proposal retains unrelated known operations and records the exact
recursive claim leaves that still require proof.

- `inlineAndTracked` and `inlineAndReturned` reach incompatible schedules for
  parameter 0, so no callback closure is asserted.
- `contradictOnZeroOnly` keeps the contradiction local to parameter 0; known
  facts elsewhere in the summary remain present, but the callback domain is
  not complete.
- `twoTrackedSites` deduplicates equivalent same-stack/tracked behavior.
- `twoParameters` keeps same-stack/untracked parameter 0 independent from
  same-stack/**tracked** parameter 1. The two differ on the *tracking* axis
  alone, which is the point: 2.0's `createEffect` runs its compute during the
  creating call, so a tracked callback is not automatically a later one.
- `oneTrackedSite` and `oneInlineSite` are single-site controls. The latter has
  a known callback operation but no guessed returned-reactive shape because
  the fixture supplies typings without an exact runtime fact for that leaf.

The emitted main document is an unaccepted proposal: absence from a `closed`
set means open knowledge, never complete-negative behavior. The sibling
proposal plan carries stable claim IDs for every local closure candidate. Only
proof replay may close one and issue a receipt.

The declaration stub transcribes the solid-js@2.0.0-rc.3 signatures used by the
fixture. `index.ts` was type-checked under `tsc --noEmit` against the audited
`solid-js@2.0.0-rc.3` install itself, not against the stub — which is the only
way the 2.0 `createEffect` split shows up, since the stub would have accepted
whatever it declared.

That split is why every `createEffect` here takes two arguments. 2.0's
signature is `createEffect(compute, effectFn, options?)` and the
single-argument form is a deprecated overload returning `never`; the tracked
invocation site stays in the compute arm and the effect arm references no
parameter, so every claim in this fixture is the one it made under 1.x.
