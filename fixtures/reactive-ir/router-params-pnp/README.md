# Yarn Plug'n'Play: no node_modules router (ADR 0324, no premise)

A PnP project has `.pnp.cjs` and no `node_modules/@solidjs/router`: the compiler resolves nothing, so there is no release or bytes to audit; the premise is not applied (the router notice says the release was not resolved) and the site stays open (`SC9012`).
