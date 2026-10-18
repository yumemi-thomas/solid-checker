# patch-package patches the router (ADR 0324 refusal, record layer)

`package.json` runs `patch-package` after install and `patches/` holds
`@solidjs+router+2.0.0-next.26.patch`. `patch-package` writes no manifest or
lockfile entry naming the router, so the record is the patch file's name in
`patches/`. The router is the published `2.0.0-next.26` files byte for byte (see `router-params`
README), so the byte digest matches the audited one and the refusal here is the
record's own, not a digest mismatch. The site `App.tsx` settles under an
unmodified install stays open (`SC9012` at the callee's argument).
