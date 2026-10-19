# An install whose bytes are not the audited release's (ADR 0324 refusal)

`dist/index.js` is the published `2.0.0-next.26` with `wrapParams` (`dist/index.js:1306`) changed so `params.id` is an object, under an unchanged manifest version. The premise checks the installed bytes (digest of manifest fields and every `dist/` file, `scripts/router-digest.mjs`) against the audited release's, is not applied, the site stays open (`SC9012`) and `SC9014` reports the differing bytes.
