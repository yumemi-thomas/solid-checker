# SC2009: an optimistic setter in an effect apply's first run (ADR 1175)

`createEffect`'s first apply runs in `render`'s synchronous tail flush (ADR 0505's premises: a static tree, a body that runs
nothing else, a compute that is one primitive literal, a module-scope `render` first), while `createOptimistic(async () => 1)`
answers a microtask later at the earliest: rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD at the setter. A violation.
