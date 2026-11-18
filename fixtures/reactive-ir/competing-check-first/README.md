# competing-check-first

**Claim (ADR 1052).** Where a check Solid makes earlier at the same call is proven to throw, the later rule's code is never reached, so its row is uncertifiable `creation-throws-first`, not a violation:

- `EffectInApply`, `NamespaceEffectInApply`: an effect created in an effect's apply. SC6002's `PRIMITIVE_IN_EFFECT_CALLBACK` throws before `NO_OWNER_EFFECT` (SC4001).
- `TrackedAndHandler`, `TrackedAndHandlerCleanup`: a helper a tracked effect and a handler both call. SC3001's refusal throws at mount and halts the system, so the handler's unowned run (SC4001) never reports.
- `MissingEffectFunctionInTracked`: SC7001's `MISSING_EFFECT_FN` throws before SC3001's refusal.

- `ApplyAndHandler`, `ApplyAndRender` (part 3): a helper an apply and a handler or a render both call. SC6002's `effect-owner-unproven` obligation names the apply caller, whose run throws `PRIMITIVE_IN_EFFECT_CALLBACK` and halts.

- `SlotCalledInApply` (part 7): a module object's slot only an apply calls; every unowned run of its effect meets the guard first.

Violations kept: `SlotAlsoInTimer` (a timer calls the slot too), `HandlerOnly` (SC4001), `EffectFunctionPresentInTracked` (SC3001), and SC6002, SC3001 and SC7001 where they are the earlier check.

Chrome: `fixtures/runtime-oracle/competing-check-*` (one case per family: a halt stops the page). Stubs: `rc21-create-in-effect-unplaced`'s layout (a package the calls resolve to), reduced from rc.21.
