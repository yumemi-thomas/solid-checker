# event-dispatch-document

Claim (ADR 0393): `document.dispatchEvent(...)` on the global `document` does not withhold the event proofs on a web release whose delegated `eventHandler` was read (rc.17): it starts at the event target and finds handler data on no element of the path. The handler's effect creation is a violation (SC4001); its write and read are clean.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
