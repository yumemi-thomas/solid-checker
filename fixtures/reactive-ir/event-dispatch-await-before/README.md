# event-dispatch-await-before

Claim (ADR 0477): a dispatch written before the function's await runs on the stack of whoever called the function, so it keeps the veto (ADR 0435): the handler's effect creation, write and read are uncertifiable.

The stub packages carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
