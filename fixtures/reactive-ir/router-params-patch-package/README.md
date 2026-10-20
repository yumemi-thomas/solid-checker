# patch-package patches the router (ADR 0324 refusal, record layer)

`package.json` runs `patch-package` and `patches/` holds `@solidjs+router+2.0.0-next.26.patch` (no manifest or lockfile entry names the router; the record is the patch file's name). The router bytes are the published `2.0.0-next.26` (digest matches), so the refusal is the record's own: the site stays open (`SC9012`).
