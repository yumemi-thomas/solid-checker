# product-realm-public-static

**ADR 1154: a second page of markup alone (`public/frame.html`) runs no code and loads nothing: the realm holds, and the seven SC2002 sites are cleared as in `product-realm-diagnostics-off` (the control of `product-realm-public-handler`).**

A twin of `product-realm-diagnostics-off` (its project, its stand-ins and its `.solid-checker/runtime.json`; the plugin's diagnostics bridge is off there), with only the page changed. **Cleared**: the seven SC2002 sites of `App.tsx` (a typed silence, no row, `package_action_calls.product-realm`).
