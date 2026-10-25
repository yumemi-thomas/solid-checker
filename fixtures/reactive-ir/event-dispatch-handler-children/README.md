# event-dispatch-handler-children

Claim (ADR 0395): the event-only proof of a handler does not depend on where its element is written, so a dispatch in the handler of an element placed in a component's children leaves the veto, as one in a handler placed in the component's own body does.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
