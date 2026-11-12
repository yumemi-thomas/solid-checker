# leaf-slot-climb-bound

**Claim (ADR 0959).** The slot climb of the leaf-owner walk follows at most sixteen layers: `untrack` runs its callback in place, so a call written in it is on the stack of the leaf owner, and the walk crosses one layer per callback. A call under seventeen of them is a climb that ran out of layers before it reached the leaf callback (`ClimbStop::Bound`): an uncertifiable row of SC3001 (`leaf-owner-unproven`), never a violation and never silence. The walk cannot say that the rest of the nest runs where it must.

`App.tsx`: `PastTheBound` (seventeen `untrack` layers around a `flush()` in a `createTrackedEffect`): one row. `AtTheBound` (sixteen: still crossed): the walk's own proof, a violation.

The two stops that owe an obligation are causes of their own in the what-if build (ADR 0965, `--assume Premise(leaf-owner-climb-bound)` and `Premise(leaf-owner-climb-unmodelled-in-place)`); this fixture is the one for the bound, `leaf-unmodelled-slot` the one for a callback the climb cannot read.
