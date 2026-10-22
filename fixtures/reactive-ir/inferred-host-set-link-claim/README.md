# inferred-host-set-link-claim

An inferred browser host (ADR 0270) proves browser execution, not that nothing server-renders, so rc.17 `setLinkClaim`'s SC9014 gap stays (declared CSR closes it: `rc17-set-link-claim-csr`). Web stub: `host-dead-return`'s manifest, `setLinkClaim` from `types/client.d.ts:131`, `dist/web.js:389`, `dist/server.js:4125`.
