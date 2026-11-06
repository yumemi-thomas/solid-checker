# store-sub-value-flow

ADR 0790: SC2003's flow reads a root no row describes. Violations: nested identity with the store itself, a function literal called in place, a conditional's or `??`'s branch.
Uncertifiable: a destructured loop binding or declarator, an entry's value, a conditional or literal root, a mutable binding, a destructured
or rest parameter, a delete through a call, an awaited promise, a walk past one frame. Silent: a literal written at its own property, a frame's
plain argument, a conditional's test (its first token named the store before ADR 0790, a false violation), a DOM call's result.
