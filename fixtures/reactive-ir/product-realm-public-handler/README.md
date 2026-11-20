# product-realm-public-handler

**ADR 1154: a second page of the project, `public/frame.html`, holds an inline handler after a solidus (`<body/onfocus="parent.hook()">`, no `import(` or `<script`: the scan read nothing else of it): another page that can run code may share a module with the page, so the realm is unknown (`other_page`). The scan of other pages needed white space before `on`.**

A twin of `product-realm-diagnostics-off` (its project, its stand-ins and its `.solid-checker/runtime.json`; the plugin's diagnostics bridge is off there), with only the page changed. **Kept**: the seven SC2002 sites of `App.tsx` keep their rows (the realm is unknown, so each site is judged against every writer of the program).
