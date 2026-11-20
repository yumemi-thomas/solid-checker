# census-claim-order (ADR 1128)

The value-use census walks every reference of a proven source (a *start*) and answers each by itself: ADR 0909's contract is that a start's
answer is a function of the build and the start alone, so the starts can run on any worker in any order. Until ADR 1128 a start could leave
work queued: the walks past a call a reached prototype patch withholds (ADR 0876, the `gp[patchKey()] = …` write at the end of the file) queue
late walks (ADR 1104, here the `for…of` binding `child`), and nothing ran them. The next start the same worker took ran them, under its own
source and memo, so which start named a row (the accessor of an SC5003 row, the first reach in start order) followed the scheduler: one run in
some number differed, and a project that crosses the threshold (64 starts) never answered twice alike.

Two copies of one start-leaving unit, so that the start a leaving start's work lands in differs by the order the starts are claimed in
(`SOLID_CHECKER_IR_CLAIM_ORDER=reverse` shows it on one worker with no scheduler). The answer is held to the same on every claim order and
worker count by `rust/crates/solid-facts-backend/tests/verdict_determinism_process.rs` (which repeats the unit past the threshold) and by
`scripts/determinism-check.mjs`. The unit was reduced from a corpus program (a tree mapper) by delta debugging while the claim orders still
disagreed: `entry`, `gp` and `patchKey` stay undeclared globals (declaring `entry` as a type or `any` hides the leak), and the empty `Tree`
and the two types stay because the difference needs them.
