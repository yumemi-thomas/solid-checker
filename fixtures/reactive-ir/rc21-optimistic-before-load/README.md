# SC2001: an optimistic setter before its source's first answer

rc.21 throws `OPTIMISTIC_WRITE_BEFORE_LOAD` from `createOptimistic(async …)`'s and
`createOptimisticStore(async …, seed)`'s setters before the first answer, ahead of
any owned-scope guard (ADR 0624). An owned write there is uncertifiable.
