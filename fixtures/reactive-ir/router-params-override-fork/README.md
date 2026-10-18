# pnpm overrides the router with a fork (ADR 0324 refusal, record layer)

`pnpm.overrides` maps `@solidjs/router` to `github:someone/router-fork#main`:
the install may be a fork that states the audited version. The router is the published `2.0.0-next.26` files byte for byte (see `router-params`
README), so the byte digest matches the audited one and the refusal here is the
record's own, not a digest mismatch. The site `App.tsx` settles under an
unmodified install stays open (`SC9012` at the callee's argument).
