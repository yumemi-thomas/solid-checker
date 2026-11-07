# symbol-key-const

ADR 0877: `globalThis[K] = v`, with `K` a `const` of this file holding `Symbol.for(...)` or `Symbol(...)`, writes no name, as the same call in place. It raises no `("", "*")`, so `symbol_open` stays shut and the made receiver of `.sort()` stays standard.

Clean of `standard-member-impostor`.
