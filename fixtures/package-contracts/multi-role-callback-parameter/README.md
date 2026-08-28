# Multi-role callback parameters stay locally open

This fixture pins the temporary-v2 replacement for the former schema-v1
unknown sentinel. A parameter reached through incompatible execution sites
does not emit contradictory callback rows and does not close the callback
domain. The proposal retains unrelated known operations and records the exact
recursive claim leaves that still require proof.

- `inlineAndTracked` and `inlineAndReturned` reach incompatible schedules for
  parameter 0, so no callback closure is asserted.
- `contradictOnZeroOnly` keeps the contradiction local to parameter 0; known
  facts elsewhere in the summary remain present, but the callback domain is
  not complete.
- `twoTrackedSites` deduplicates equivalent queued/tracked behavior.
- `twoParameters` keeps same-stack/untracked parameter 0 independent from
  queued/tracked parameter 1.
- `oneTrackedSite` and `oneInlineSite` are single-site controls. The latter has
  a known callback operation but no guessed returned-reactive shape because
  the fixture supplies typings without an exact runtime fact for that leaf.

The emitted main document is an unaccepted proposal: absence from a `closed`
set means open knowledge, never complete-negative behavior. The sibling
proposal plan carries stable claim IDs for every local closure candidate. Only
proof replay may close one and issue a receipt.

The Solid 1.x declaration stub transcribes the primitive signatures used by
the fixture and remains clean under `tsc --noEmit`.
