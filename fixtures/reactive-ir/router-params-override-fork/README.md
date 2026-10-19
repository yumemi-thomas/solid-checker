# pnpm overrides the router with a fork (ADR 0324 refusal, record layer)

`pnpm.overrides` maps `@solidjs/router` to `github:someone/router-fork#main` (the install may be a fork stating the audited version). The router bytes are the published `2.0.0-next.26` (digest matches), so the refusal is the record's own: the site stays open (`SC9012`).
