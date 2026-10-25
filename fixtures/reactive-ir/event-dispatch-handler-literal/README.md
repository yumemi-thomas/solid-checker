# event-dispatch-handler-literal

Claim (ADR 0400): a dispatch written in a function literal handed to an exported helper that calls its parameter runs where the call that handed it over runs. The helper's only call is in the first handler, so the literal's only invocation is an event delivery (ADR 0395's census, now following the export through the program's other modules): the dispatch leaves the veto, and the second handler's effect creation is a violation (SC4001) with a clean write and read. `variants/body`: the same helper is also called from a component's body, an owned entry, so the dispatch keeps the veto. `variants/stored`: a helper that keeps its argument leaves it unplaced.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
