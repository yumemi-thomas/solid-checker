# A package callback run as an owned computation

ADR 0183: an accepted contract can state that an export runs a callback argument on every call, during the call, tracked, under an owner the call creates (a memo/effect compute), so a signal write in a literal at that argument throws `REACTIVE_WRITE_IN_OWNED_SCOPE` (`SC2001`); the queued effect function and returned closures are none; `min: 0` and no-owner-claim exports are `SC9005`. Summaries are the generator's output for `fixtures/package-contracts/owned-computation-callbacks`; the `solid-js` stub is verbatim rc.9.
