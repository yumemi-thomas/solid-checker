# event-dispatch-window

Claim (ADR 0393): `window.dispatchEvent(...)`, where `window` is the global declared by `lib.dom.d.ts`, reaches only `window`'s own listeners, never a JSX handler, so it does not withhold the event proofs: the handler's effect creation is a violation (SC4001); its write and read are clean.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
