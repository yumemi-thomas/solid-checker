# event-dispatch-await-continuation

Claim (ADR 0477): a dispatch written in an async function after an `await` of that function which dominates it (ADR 0320) runs on a continuation the microtask queue resumes from an empty stack, with no owner and no strict-read window, so it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean. The fuzzer's entry module (`scripts/fuzz/checker.mjs` `MAIN_SOURCE`) drives handlers this way.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
