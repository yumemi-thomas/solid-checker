# event-dispatch-handler-open

Claim (ADRs 0437, 0640): the census needs a closed program, but one inline handler's only invocation is a delivery in an open program too, so its dispatch leaves the veto: the sibling handler's write and read are clean and its effect creation is the violation.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; no `.solid-checker/runtime.json` closes the boundary.
