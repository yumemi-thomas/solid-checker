# event-dispatch-await-conditional

Claim (ADR 0477): an await under a condition does not dominate the dispatch after it (the call `exercise(false)` never suspends), so the dispatch may run on its caller's stack and keeps the veto: the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
