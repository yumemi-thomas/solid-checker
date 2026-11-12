# owner-call-component-by-reference

**Claim (batch 11, slice 6; ADR 0969; the review of #220, F1).** A function that returns JSX is entered by renders alone only where every reference of its binding is a JSX tag. The owner graph gives such a function an owned context, and the emission's test of what enters it (`named_layer_entry_open`, ADR 0868) stops at a function the census proves a component before it looks at the references: the component-render premise, which the ruling for the owner-call universe excludes. The universe's silences (`OwnerCurrent` for SC6002, `LeafOwnerAbsent` for SC3001) ask the test through `layer_entry_open_by_reference`, which takes no such premise: a component a reference enters (`pick = view`, then `pick!()` in an apply) is a layer like any other, and its creation keeps the uncertifiable row.

| site | outcome | why |
|---|---|---|
| `createMemo` in `Tagged` | silent (`effect-creation-runs-off-apply`, `owner-context-no-leaf`) | only a tag renders it: its entries are renders, rendering runs it under the owner its render sets |
| `createMemo` in `view`, picked by `Picks`'s apply | SC6002 `effect-creation-unaccounted` (and SC3001's two codes: the program names a leaf opener); decline `entry-by-reference` | a function that returns JSX, no tag renders it, `pick = view` enters it by reference and the apply calls it: `context === null` on the apply's stack (Chrome rc.21) |
| `createMemo` in `Rendered`, a tag and `pickRendered = Rendered` | the same | a component a tag renders and a reference also enters |
| `createMemo`, `onCleanup` in `Leaf`, called by `createTrackedEffect`'s callback | SC3001 `leaf-operation-unaccounted` each, SC6002 for the creation; decline `entry-by-reference` | the leaf owner's callback runs the function a reference picked: PRIMITIVE_IN_FORBIDDEN_SCOPE and CLEANUP_IN_FORBIDDEN_SCOPE (Chrome rc.21) |

Chrome (`fixtures/runtime-oracle/owner-call-component-by-reference`, the same program with every component exported): the three picked functions throw at their creations (and at `Leaf`'s `onCleanup`); `TaggedOnly` is quiet. Before the fix the checker certified the picked creations (the head of #220 ended them with `owner-context-current` and `owner-context-no-leaf`, and the base build had no row either); now each is covered by an uncertifiable row.
