# caught-read-kept-closure

**Claim (ADR 1176).** A caught read (`try { a1(); } catch {}`) written in a `const`-bound local closure, neither `async` nor a
generator, with no nested function, that the computation calls in place, is the computation's caught read when the closure reads
nothing outside a kept block (`local_access::call_of_kept_closure`): its call is no other throw of the pass, so SC5008 is proven
where the direct read is (ADR 0637). rc.21 reports CAUGHT_REACTIVE_READ.

Violations: `MemoClosure`, `EffectClosure`. Uncertifiable: `TwinReadOutside` (an uncaught read in the closure may leave the pass
first), `TwinNested` (a nested function). Chrome: `runtime-oracle/caught-read-kept-closure`. Stub: `caught-read-through-helper`'s.
