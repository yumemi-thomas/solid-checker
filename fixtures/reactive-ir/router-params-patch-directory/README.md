# A patches/ entry names the router (ADR 0324 refusal, record layer)

`patches/@solidjs+router+2.0.0-next.26.patch` exists with no tool named in
`package.json`. A patch directory entry naming the router is a record whatever
applies it. The router is the published `2.0.0-next.26` files byte for byte (see `router-params`
README), so the byte digest matches the audited one and the refusal here is the
record's own, not a digest mismatch. The site `App.tsx` settles under an
unmodified install stays open (`SC9012` at the callee's argument).
