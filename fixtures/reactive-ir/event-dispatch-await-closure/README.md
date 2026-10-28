# event-dispatch-await-closure

Claim (ADR 0477): a dispatch in a closure written after the await belongs to the closure, which runs on the stack of whoever calls it, not on the continuation: it keeps the veto, and the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
