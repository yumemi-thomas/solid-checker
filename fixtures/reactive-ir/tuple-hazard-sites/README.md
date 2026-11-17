# tuple-hazard-sites

**Claim (ADR 1027).** The hazard table names where each disjunct of ADR 0803's slot withdrawal holds, so a `tuple-iteration-written` row can be traced to the code that fires it. One file per census: `visible.ts` writes an iterator key on a parameter (`VisibleIterationWrite`, cause `visible-write`); `obtained.ts` obtains the array iterator's prototype (`TupleSlotIterationMayBeMutated`, `intrinsic-obtained`); `evaluated.ts` calls `eval` directly (`eval`, and the name read as a value, `intrinsic-obtained`); `handed.ts` hands `window` to a function no source declares (`intrinsic-handed`). `Counter`'s signal slots are withdrawn, so its reads and its setter call are SC9012 `tuple-iteration-written` rows. Diagnostics only: the rows are the same with or without the table.

Twin: `tuple-hazard-sites-shadowed`. Pinned by `the_tuple_iteration_hazards_name_their_sites` (diagnostics_process).
