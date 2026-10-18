# The router is a link: dependency (ADR 0324 refusal, record layer)

`dependencies["@solidjs/router"]` is `link:../router`: whatever sits there is
the project's own checkout, not a registry install. The router is the published `2.0.0-next.26` files byte for byte (see `router-params`
README), so the byte digest matches the audited one and the refusal here is the
record's own, not a digest mismatch. The site `App.tsx` settles under an
unmodified install stays open (`SC9012` at the callee's argument).
