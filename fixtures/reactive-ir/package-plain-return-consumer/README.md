# A consumer of a `plain` return

ADR 0113: an accepted contract closing `returns` over one `plain` return leaves nothing open at the import (a plain value carries no reactive capability, so it projects to "no reactive return described"); the same export with `returns` open raises `SC9005`. `Status`/`isReady` clean, `StatusOpen`/`isReadyOpen` `SC9005`. Ships `.solid-checker/authorize-contract.json` (as `../package-merged-props-consumer`); manifest byte-identical to that fixture's; `node_modules/solid-js` required for the authorized copy.
