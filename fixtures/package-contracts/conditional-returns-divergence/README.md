# Conditional return divergence

`Show` returns a reactive accessor leaf in the browser artifact and a plain
value in the node artifact. `Steady` is the control whose return behavior is
equivalent in both.

Temporary-v2 generation models finite runtime/declaration selections as exact
artifact cases, not a base summary plus wire variants. One branch's positive
return and another branch's complete negative are distinct semantics; unresolved
selection may join possible behavior monotonically but cannot invent a
guaranteed accessor or a global negative.

The generator now retains the exact default, node, and browser cases. The
synthetic contradictory `browser,node` selection remains uncertifiable and is
pinned in `expected-refusals.json`; it no longer erases the independently
known branches or opens an unrelated claim domain.
