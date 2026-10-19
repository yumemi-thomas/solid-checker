# fp-summary-cleanup-callback

**Claim.** A read inside an `onCleanup` callback is not a strict-window read: the callback is registered on the owner and runs on disposal or re-run, so it is `Deferred` as `onSettled`'s is (`Solid2::callback_executions`, `solid_2.rs`; rc.9 dev: body reads raise `STRICT_READ_UNTRACKED`, cleanup reads do not). Body reads and snapshots captured by the callback stay `SC1001` violations. `solid-js.d.ts` copies `createSignal`, `createMemo`, `Disposable`, `onCleanup` verbatim from rc.9; `tsc` strict against stub and real rc.9.
