# product-realm-page-handler-solidus

**ADR 1154: the same handler after a solidus, `<body/onload=…>`. HTML's tokenizer separates attributes by white space and by a `/` not followed by `>`, so it is the handler it is with a space before it (Chrome rc.21 runs it). The grammar read `/onload` as the attribute's name, passed the page, and the realm narrowed: on main 5e9f20381 the seven SC2002 sites were cleared though the page loads the writer (a false clean, ADR 1154).**

A twin of `product-realm-diagnostics-off` (its project, its stand-ins and its `.solid-checker/runtime.json`; the plugin's diagnostics bridge is off there), with only the page changed. **Kept**: the seven SC2002 sites of `App.tsx` keep their rows (the realm is unknown, so each site is judged against every writer of the program).
