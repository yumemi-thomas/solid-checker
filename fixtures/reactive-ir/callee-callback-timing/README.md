# callee-callback-timing

**Claim.** A read in a function literal in a component body handed to a *project* function (directly or in an array/object literal argument) is `SC1001` **uncertifiable** unless the callee's own body calls that parameter during the call, then a proven violation (`execution_role::callee_callback_timing`); no-body callees prove it only through a contract `inline` row. `Props.tsx`/`Async.tsx` (`SC5001`) apply it to prop callbacks and pending reads. Stub holds `createSignal` verbatim from rc.9.
