# The router is a link: dependency (ADR 0324 refusal, record layer)

`dependencies["@solidjs/router"]` is `link:../router` (the project's own checkout, not a registry install). The router bytes are the published `2.0.0-next.26` (digest matches), so the refusal is the record's own: the site stays open (`SC9012`).
