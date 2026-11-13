# owner-call-saved-callback

**Claim (batch 11, slice 6; ADR 0969; the review of #220, F2).** The role engine places a handler, a timer callback or the body of a component off every effect's apply from where the function is written. It does not follow a function that is also stored (a variable, an array) and called from an apply, so the lexical role alone cannot end SC6002's creation. `EffectCreationRunsOffApply` is therefore produced by the owner stage (`owners/leaf_universe.rs`) and rests on the role AND on the facts every silence of the universe asks: the site is plain (not in a generator, a parameter list or after an await), no package the build does not read may run the function (B3, ADR 0967), and no layer around the site is entered by a reference that is no direct call (`layer_entry_open_by_reference`: a function value no binding names that is neither run in place nor a whole argument of a call, or a binding with any other reference).

| site | outcome | why |
|---|---|---|
| `createMemo` in the handler `Saved` also stores in `saved` | SC6002 `effect-creation-unaccounted`; decline `no-edge-into-function` | an assignment is an expression, so the attribute still yields the arrow, which `saved!()` in an apply also runs (Chrome rc.21) |
| `createMemo` in the timer callback `SavedTimer` stores | the same | the same through `setTimeout` |
| `createMemo` in `InArray`'s `handler`, also an array's element | SC6002 `effect-creation-unaccounted`; decline `unowned-entry` | `all[0]!()` in an apply calls it: no edge shows the array element's call |
| `createMemo` in the generator `StepGenerator` steps | the ADR 0710 obligation row; decline `generator-body` | a generator's body runs where the apply steps the iterator |
| `createMemo` in `untrack(function* …)`, `StepUntrack` | SC3001 and SC6002 `…-unaccounted`; decline `unread-package-may-run` | `it.next` is a standard method no census follows, so a package the build does not read may run the generator (B3) |
| `createMemo` in `Timer`'s `setTimeout(() => …)` | silent (`effect-creation-runs-off-apply`) | a whole argument of the call: nothing else holds it, and the timer runs it on a fresh stack |
| `createMemo` in `Body` | silent | a component only a tag renders |
| `createMemo` in `Handler`'s inline `onClick={() => …}` | SC6002 `effect-creation-unaccounted`; decline `unowned-entry` | **the cost**: the entry test reads a function value no binding names that is not a whole argument of a call as an object slot or an array element, so an inline handler is no closed layer. Chrome is quiet there; the proof batch's refinement is a literal the compiler binds as the element's event handler, entered only by the event |

Chrome (`fixtures/runtime-oracle/owner-call-saved-callback`, every component exported): the five saved or stepped creations throw PRIMITIVE_IN_EFFECT_CALLBACK; `Timer` and `Body` are quiet. Before the fix the checker certified the five (the head of #220 ended each with `effect-creation-runs-off-apply`, and the base build filed no row either); now each is covered by an uncertifiable row.
