# Conditional return divergence

`Show` returns a reactive accessor leaf in the browser artifact and a plain
value in the node artifact. `Steady` is the control whose return behavior is
equivalent in both.

Temporary-v2 generation models finite runtime/declaration selections as exact
artifact cases, not a base summary plus wire variants. One branch's positive
return and another branch's complete negative are distinct semantics; unresolved
selection may join possible behavior monotonically but cannot invent a
guaranteed accessor or a global negative.

The generator retains the exact default, node, and browser cases. Its condition
census treats `browser` and `node` as mutually exclusive runtime-target values,
so it does not invent a synthetic combined selection. The independently known
branches remain separate and no unrelated claim domain opens.
