# tuple-hazard-sites-shadowed

**Claim (ADR 1027), the shadowed twin of `tuple-hazard-sites`.** The same component, beside a file that binds `Function`, `Array` and `window` locally (the census reads globals, so a local of the same name obtains nothing) and hands the local `window` on, and a file that writes `Symbol.iterator` on an object literal it made (proven separate from the array intrinsics). No slot is withdrawn: no `tuple-iteration-written` row, and the hazard table names neither `VisibleIterationWrite` nor `TupleSlotIterationMayBeMutated`.
