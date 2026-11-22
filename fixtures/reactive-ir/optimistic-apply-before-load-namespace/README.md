# SC2009: the namespace twin (ADR 1175, a stated gap)

The same write as `optimistic-apply-before-load` through `import * as S`: Chrome throws, but ADR 0505's body premise reads no
member, so `S.createOptimistic` and `S.createEffect` leave it uncertifiable (`optimistic-reach-unresolved`), never clean.
