# event-dispatch-timeout

Claim (ADR 0393): a dispatch written in the callback a host scheduler (`setTimeout`) runs from an empty stack has no owner and no strict-read window, so it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
