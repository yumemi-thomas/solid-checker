# owner-call-root-generator

**Claim (batch 11, slice 6; ADR 0969; the review of #220, F3).** `EffectCreationOwnerSetAround` ends a creation written in the callback of an exact primitive that creates an owner (`createRoot`, a computation's compute), through the layers that run in place and add no owner. It asked nothing of the layers themselves. Each layer the climb crosses must run its code in the primitive's synchronous run (`owner_created_around_call`): a generator's body runs where an iterator is stepped, after the root's owner is gone; a parameter default runs where a call omits the argument; code after an `await` of an async layer runs from a later task. None has the owner the primitive set, so none is a plain site and none is ended here (the owner stage's `generator-body` decline is the same question).

| site | outcome | why |
|---|---|---|
| `createMemo` in `createRoot(function* () { … })`, stepped by an apply (`RootGenerator`) | SC6002 `effect-creation-unaccounted`; decline `generator-body` | the body runs on the apply's stack with no owner: PRIMITIVE_IN_EFFECT_CALLBACK (Chrome rc.21) |
| `createMemo` in `createRoot(() => { … })` inside a memo (`RootPlain`) | silent (`effect-creation-owner-set-around`) | a plain callback runs under the owner the root creates |
| `createMemo` after the `await` of `createRoot(async () => { … })` (`RootAsync`) | SC6002 `effect-creation-unaccounted`; decline `after-suspension` | the continuation runs from a later task, outside the root's run. Chrome is quiet (no apply is on that stack): the cost of not proving which stack it is |

Chrome (`fixtures/runtime-oracle/owner-call-root-generator`): `RootGenerator` throws at the creation, `RootPlain` is quiet. Before the fix the head of #220 ended the generator's creation with `effect-creation-owner-set-around`; now it is covered by an uncertifiable row.
