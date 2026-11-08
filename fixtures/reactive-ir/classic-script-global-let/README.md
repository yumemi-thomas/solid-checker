# classic-script-global-let

**Claim (ADR 0904).** A classic script's top-level `let Promise` is the global lexical binding every module resolves through, so `Promise.resolve().then(cb)` in the module is no standard promise: `("", Promise)` vetoes it. Before, the declaration raised no patch and `then` was read as the standard later run.
