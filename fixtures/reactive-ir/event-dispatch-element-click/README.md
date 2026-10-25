# event-dispatch-element-click

Claim (ADR 0356, 0393): an element `click()` written in a component body is a synchronous dispatch on an unproven stack, so it withholds every event-only owner, write and read proof of the program: the handler's effect creation, write and read are uncertifiable, none is a violation.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
