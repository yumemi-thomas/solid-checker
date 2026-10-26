# event-dispatch-module-scope

Claim (ADR 0435): a dispatch at module top level runs before any component, owner or strict-read window exists, so it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
