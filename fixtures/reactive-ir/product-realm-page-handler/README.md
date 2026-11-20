# product-realm-page-handler

**ADR 1154: `index.html` holds an inline handler after a space, `<body onload="import('/node_modules/fz-prealm-any/index.js')">`: the page loads the package that writes the built-in, and the grammar refuses the handler, so the realm is unknown (ADR 1145 reads no entry set) and the seven SC2002 rows stay. The control of the three that follow.**

A twin of `product-realm-diagnostics-off` (its project, its stand-ins and its `.solid-checker/runtime.json`; the plugin's diagnostics bridge is off there), with only the page changed. **Kept**: the seven SC2002 sites of `App.tsx` keep their rows (the realm is unknown, so each site is judged against every writer of the program).
