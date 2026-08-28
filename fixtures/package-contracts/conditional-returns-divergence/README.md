# Conditional return divergence

`Show` returns a reactive accessor leaf in the browser artifact and a plain
value in the node artifact. `Steady` is the control whose return behavior is
equivalent in both.

Temporary-v2 generation models finite runtime/declaration selections as exact
artifact cases, not a base summary plus wire variants. One branch's positive
return and another branch's complete negative are distinct semantics; unresolved
selection may join possible behavior monotonically but cannot invent a
guaranteed accessor or a global negative.

This synthetic package does not provide enough exact artifact authority for the
generator to prove and partition those branches, so Phase 14 records a focused
refusal in `expected-refusal.txt`. The known fact about either branch is not
discarded or silently selected, and no unrelated claim domain is opened.
