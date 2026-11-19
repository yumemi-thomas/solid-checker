# sc1001-helper-member

**Claim (ADR 1121).** A store member read in a helper the component body calls is the body's read: SC1001 at the call, as for a `createStore` slot (`r()` propagates `store.v`). Two shapes were certified clean (no row of any rule) because the interprocedural summaries never carried them:

- a store a primitive returns whole (`const store = createProjection(...)`, `ProjHelper`, `ProjTwoLevel`, `ProjDraftHelper`) or a hook returns whole (`HookProjHelper`): only tuple slots and accessors entered `summary_source_symbols`;
- a receiver under a TypeScript wrapper (`(store as S).v`, `store!.v`, `(store satisfies S).v`: `AsHelper`, `NonNullHelper`, `SatisfiesHelper`): the summary matched a member's unpeeled object against the reference span.

Violations: every component above. Silent: `ProjHelperInMemo` (the helper runs in a memo's compute), `ProjHelperUntracked` (`untrack(r)`). Chrome (`runtime-oracle/sc1001-helper-member`): STRICT_READ_UNTRACKED at the seven reads, nothing at the two negatives. Stub: `absent-guard-prop`'s (rc.21 `createProjection(fn, seed): Store<T>`, `createStore`, `createMemo`, `untrack`).
