# primitive-indirect-call

**Claim (ADR 0902).** A Solid primitive called through `f.bind(…)(…)`, `f.call(…)`, `f.apply(…)`, a `const` holding a bound `f`, or a logical-assignment callee (`(slot ||= f)(…)`) gets the claiming rule's uncertifiable row at the call, as the direct call does: an attribution, never a proof (a replaced `call`/`bind`, or the slot's earlier value, may run instead).

`run` is called by the body (owned) and later by a click with no owner, so each `onSettled` form there is SC4001 uncertifiable (`owner-callers-unenumerated`), as `direct`'s `onSettled(…)` is. For `call`/`apply` the row is at the callee, since the callback follows the receiver. `flush.call(…)` in a timer gets `flush`'s ADR 0884 rows (SC2006, SC2008). `Shadow`'s local `onSettled` is no primitive: no SC4001 row.

Stubs: `onSettled` matches `@solidjs/signals@2.0.0-rc.21` `dist/types/signals.d.ts:737`; the others are reduced to the overload used here.
