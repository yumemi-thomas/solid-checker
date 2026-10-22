# A router config naming `links` (ADR 0324 refusal)

next.41 calls `config.links(router, basePath)` with the router context (`claims.js:42-44`), whose `wrapParams` each route context reads; `router.ts` replaces it through a computed key. A config naming `links` withholds the premise on every release (`SC9012` stays). Router bytes as in `router-params`.
