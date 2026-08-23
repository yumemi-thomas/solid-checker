# An export name present in only one conditional branch

`observe` exists in the `browser` branch and not in the `node` one. Schema v1
cannot say "not exported here", so the generator must keep the branch it *was*
proven for as a variant rather than publishing it as an unconditional summary.
A node consumer then fails closed on `observe` instead of inheriting the
browser branch's callback semantics.

`shared` is the negative control: it exists in both branches with the same
semantics, so it stays unconditional and gains no variant.
