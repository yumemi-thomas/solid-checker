# event-dispatch-handler-inline

Claim (ADR 0395): an `HTMLElement.click()` written directly in an inline `onClick` handler of an intrinsic element runs on the stack of an event delivery, which has no owner and no strict-read window, so it does not withhold the event proofs: the sibling handler's effect creation is a violation (SC4001); its write and read are clean.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary, which the event-only census needs.
