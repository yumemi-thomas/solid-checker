# effect-cleanup-row-helper-read

**Claim (ADR 1198).** A call of a project helper or a hook's member accessor, written in the cleanup an inline apply returns where the compute tracks nothing (ADR 1167), runs at the effect's disposal only, outside any strict-read window: the reads its summary carries are silent (`ShowWrapper`, `ForMember`; uncertifiable on main since ADR 1196, `function-run-unplaced`). The answer is the strict read's summary rows only: `TrackedCompute` (a rerun runs the cleanup in the apply's window) stays uncertifiable, and `ForActionValues` keeps the dispatch row at the call that ADR 1196 restored (a `For` row's owned disposal throws ACTION_CALLED_IN_OWNED_SCOPE).

Chrome: `fixtures/runtime-oracle/effect-cleanup-row-helper-read`. Stubs: `effect-cleanup-for-action-dispatch`'s. `tsc --noEmit` clean.
