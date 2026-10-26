# event-dispatch-unplaced

Claim (ADR 0435): a dispatch in a function nothing places (here a callback handed to a project-declared `it` (`App.tsx:19`), which is not a reviewed runner, ADR 0438) is not proven ownerless: unresolved is not evidence, so the program-wide veto stays: the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
