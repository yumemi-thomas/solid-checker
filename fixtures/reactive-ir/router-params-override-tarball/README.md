# npm overrides the router with a tarball (ADR 0324 refusal, record layer)

`overrides` maps `@solidjs/router` to a tarball URL, not a registry version. The router bytes are the published `2.0.0-next.26` (digest matches), so the refusal is the record's own: the site stays open (`SC9012`).
