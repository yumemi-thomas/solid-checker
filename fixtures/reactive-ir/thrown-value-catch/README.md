# thrown-value-catch

ADR 0923: a thrown source's untracked calls in its `catch` are `SC9012` `value-thrown` at the call; a default-exported literal or call is walked into its importers. Positive: `read*`, `NamespaceThrown`, `ThrownFromIife`. Negative: memo twins, `Shadowed`. `ThrownByCallee`, `CaughtPattern`: row at `throw`.
