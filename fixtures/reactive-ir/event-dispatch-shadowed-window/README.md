# event-dispatch-shadowed-window

Claim (ADR 0393): the exclusion resolves `window` through its declaration. A parameter named `window` is no global, so `window.dispatchEvent(...)` keeps the program-wide veto: the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
