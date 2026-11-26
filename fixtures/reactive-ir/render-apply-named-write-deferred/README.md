# SC2001: a named render-effect apply (ADR 1227)

`defer` skips the first apply (rc.21 `dist/dev.js:2628-2629`), so nothing runs under the owner: no SC2001 row (the later runs are the effect phase's).

Chrome: `fixtures/runtime-oracle/render-apply-named-write`.
