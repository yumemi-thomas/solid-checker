# SC2009: a compute that reads the source holds the apply (ADR 1175)

`createEffect(() => o(), …)`: the effect waits for the source's answer, so its apply runs after it (Chrome is silent):
uncertifiable. `Shadowed` writes through a local `createOptimistic` that is no primitive: nothing of this rule.
