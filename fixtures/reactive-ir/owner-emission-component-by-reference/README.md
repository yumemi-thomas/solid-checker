# owner-emission-component-by-reference

**Claim (batch 11, the follow-up to the review of #220; ADR 0868 amended).** The owner emission asks `named_layer_entry_open` whether a layer around an owner-requiring operation (`onCleanup`, an effect, a context read) may be entered by a route the owner graph holds no edge for, so that the owned context its edges give is no proof of every run. The walk stopped at a function the census proves a component (`function_proven_component`: it returns JSX and no call enters it; a function a reference also enters, an *escaped* component, included) before it looked at the references: the component-render premise the ruling for the owner-call universe excludes (the review of #220, F1). The graph gives such a function an owned context, so the operation was certified, no row, although Chrome rc.21 reports NO_OWNER_CLEANUP / NO_OWNER_EFFECT / NO_OWNER_CONTEXT where a timer, a handler or an effect's apply calls it through a variable. A component is now entered by renders alone only where every reference of its binding is a JSX tag (the opening and the closing name are one reference); a reference that enters it (`pick = view`, a prop, an array element) makes it a layer like any other, and the operation keeps an uncertifiable SC4001 row (`owner-callers-unenumerated`).

| site | outcome | why |
|---|---|---|
| `onCleanup` in `Tagged` | silent | only a tag renders it: its render sets the owner |
| `onCleanup` in `viewTimer`, `viewHandler`, `viewApply` | SC4001 uncertifiable | a function that returns JSX, no tag renders it, a timer, a handler or an apply calls it through a variable: no owner is current (Chrome: NO_OWNER_CLEANUP) |
| `createEffect` in `viewEffect` | SC4001 uncertifiable | the same for an effect created with no owner (NO_OWNER_EFFECT) |
| `onCleanup` in `Both`, rendered as `<Both />` and called by a timer | SC4001 uncertifiable | an escaped component: the render is one entry, the timer's call another with no owner |
| `onCleanup` in `viewHeld`, held and never called | SC4001 uncertifiable | the twin: Chrome is quiet (no cleanup runs); the row is the cost of not proving that no reference calls it |

Chrome (`fixtures/runtime-oracle/owner-emission-component-by-reference`, every component exported, a context read added): main is `unproven-clean` at five sites (NO_OWNER_CLEANUP ×3, NO_OWNER_EFFECT, NO_OWNER_CONTEXT); the fix is `covered-uncertifiable`.
