# event-dispatch-detached-owner

Claim (ADR 0393): a dispatch in the callback of `runWithOwner(null, ...)` runs with no owner, so it does not withhold owner and write proofs (the effect creation is a violation, SC4001; the write is clean), but `runWithOwner` leaves the enclosing strict-read window open, so a handler read stays uncertifiable (SC1001).

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
