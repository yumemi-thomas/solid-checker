# SC2009: an optimistic setter before its source's first answer

rc.21 throws `OPTIMISTIC_WRITE_BEFORE_LOAD` from `createOptimistic(async …)`'s and
`createOptimisticStore(async …, seed)`'s setters before the first answer, ahead of
any owned-scope guard. The pass that creates the source proves it; a handler may run
before the answer or after it, so it stays open (ADR 0636). A declared `loadingValue`
is an answer, and a sync value form has one.
