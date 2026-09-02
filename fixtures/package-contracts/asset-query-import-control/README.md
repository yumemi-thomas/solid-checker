# The control for the query-suffixed asset import

This is `asset-query-import` with the `?raw` import and its export removed, so
`./notify.js` is reached only as a module. It exists to pin the *control* half
of that fixture's claim: the sibling README says the query-suffixed entry
"carries twenty open claims and no proof candidate, where the same entry
without the `?raw` import carries three candidates", and until this fixture that
control number was prose only. A regression that flattened the control -- one
that stopped resolving the plain module import, or stopped producing proof
candidates for it -- would leave the `?raw` fixture's snapshot untouched and
silently turn its whole claim into a tautology.

`expected-proposal.json` therefore pins the exact control: **3 closure
candidates and 7 open claims** for the one export, against the sibling's 0
candidates and 20 open claims for two exports. Nothing about this fixture is
bundler-mediated; the two must not converge.
