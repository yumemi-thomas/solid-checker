# write-climb-depth

**Claim.** The write-legality climbs are bounded (16 layers, `detached_in_place`
in `write_roles.rs`), and a bound reached proves nothing: the layers past it are
unread, so the write is unplaced (uncertifiable), never the handler's lexical
role (ADR 0532). rc.17 throws `REACTIVE_WRITE_IN_OWNED_SCOPE` for a signal write
under a `createRoot` body in a handler at every depth (`@solidjs/signals`
`dist/dev-shared.js:5619-5636`; probes d11, d9, d2c, d2b, d2 of the 2026-11-01
saturation research, hit-counted). Before ADR 0532 the ladder was: a violation
at 0 and 8 IIFE layers, an obligation at 15 (the disposal climb's bound), and
silence at 16 and 17.

Cases, all `<button onClick={() => createRoot(() => { … setW1(7) … })} />`
with N IIFE layers around the write: `RootZero` (violation), `RootEight`
(violation), `RootFifteen` (obligation: `runs_in_disposal_within` exhausts),
`RootSixteen` and `RootSeventeen` (obligation: the owner climb exhausts).

Legal twins, the same layers directly in the handler with no owner:
`HandlerEight` is clean; `HandlerSeventeen` is an obligation, the recorded cost
of the bound (docs/open-work.md § 3).

`RootHelperSixteen` checks the synchronous project-helper variant (a violation through exact helper call entries); `FlowReadSixteen` checks that shared read-role consumers retain an obligation when a climb exhausts. The source passes `tsgo --noEmit` with strict checking against the real published rc.17 packages, without this fixture's declarations.
