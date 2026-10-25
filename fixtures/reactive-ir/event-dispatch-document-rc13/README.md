# event-dispatch-document-rc13

Claim (ADR 0393): the `document` exclusion needs a web release whose `eventHandler` was read for it. This project resolves the rc.13 triple, whose `eventHandler` this change did not read, so `document.dispatchEvent(...)` keeps the program-wide veto: the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.13) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
