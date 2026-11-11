# handed-root-patch-definer-key-same

ADR 0951: `Number` handed to a helper that defines `isFinite` under the literal key `"isFinite"` replaces the member the
program calls: the call stays withheld (SC9012 `standard-member-impostor`, `ReflectiveDefinition(isFinite)`), and the body's
read of `r1` an `isFinite` that runs its argument would make untracked stays uncertifiable. Chrome: STRICT_READ_UNTRACKED.
`Object.defineProperty` itself is not withheld (the key it defines is `isFinite`, not `defineProperty`).
