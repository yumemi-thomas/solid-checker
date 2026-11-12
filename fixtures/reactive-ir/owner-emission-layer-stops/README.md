# owner-emission-layer-stops

**Claim (batch 11, the follow-up to the review of #220; ADR 0970).** The owner emission (SC4001) asks `named_layer_entry_open` whether a layer around an owner-requiring operation (`onCleanup`, an effect, a context read) may be entered by a route the owner graph holds no edge for, so that the owned context its edges give is no proof of every run. It adopted the premises the owner-call universe gave the same walk: a component is entered by renders alone only where every reference of its binding is a JSX tag (fixture `owner-emission-component-by-reference`), and two stops of it that let a layer through (N1 and N2 of the check of B1 on #220):

- **N2, the stop at an owner-creating layer returned before the function's references.** `createRoot(function f() { if (!saved) { saved = f; return; } onCleanup(..) })`: the root runs the callback at once and it saves itself, a later `saved()` from a timer, a handler or an effect's apply runs it on its own stack under no owner. A callback an owner-creating primitive runs (or a control-flow child) is now entered by that position alone only where nothing references it (`referenced_only_by_entry`).
- **N1, the caller walk took a generator's body for a placed run.** A layer that is a generator (and, through the caller walk, any function its body calls) is an open entry: its body runs where an iterator is stepped. The emission already answered a helper called from a generator's body on main (its generator-step obligation), so the cell stays uncertifiable on both builds.

| site | outcome | why |
|---|---|---|
| `onCleanup` in `SelfSaving`'s root callback `f`, which saves itself and is called by a timer | SC4001 uncertifiable (`owner-callers-unenumerated`) | N2: no owner is current at the timer's call (Chrome: NO_OWNER_CLEANUP). **The false clean this fixes**: main certified it |
| `onCleanup` in `StepHelper`'s `helper`, called from the generator a timer steps | SC4001 uncertifiable | N1: no owner is current where the iterator is stepped (Chrome: NO_OWNER_CLEANUP); uncertifiable on main too |
| `onCleanup` in `NamedUnreferenced`'s root callback, a name nothing references | silent | the stop holds: only the root enters it |
| `onCleanup` in `PlainCaller`'s `helper`, called from a plain function the body calls | silent | no generator and no reference but the call: rendering enters it |

Chrome (`fixtures/runtime-oracle/owner-emission-layer-stops`, every component exported): main is `unproven-clean` at the self-saving root; the fix is `covered-uncertifiable`.
