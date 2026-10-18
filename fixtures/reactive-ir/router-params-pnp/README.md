# Yarn Plug'n'Play: no node_modules router (ADR 0324, no premise)

A PnP project has a `.pnp.cjs` and no `node_modules/@solidjs/router`. The
compiler resolves nothing for the specifier, so there is no release and no
bytes to audit: the premise is not applied (the router notice says the release
was not resolved) and the site `App.tsx` settles under an installed router
stays open (`SC9012` at the callee's argument). No router package is present.
