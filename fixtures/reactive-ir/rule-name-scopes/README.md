# rule-name-scopes

Claim (ADR 0384): `no-await-primitive-in-tracked-scope` covers `SC2004` (`resolve`) and `SC2005` (`until`), and its former names cover one code each. `App.tsx` holds one tracked compute with both calls, so this project (no rule-options) reports both. Each variant's `.solid-checker/rule-options.json` disables one name: `resolve-in-tracked-scope` or `resolve-in-reactive-scope` keeps only `SC2005`, `until-in-tracked-scope` keeps only `SC2004`, and the current name disables both.

The `solid-js` stubs carry the rc.9 version that selects the vocabulary with `until` (the same as `rc9-until-scope`, whose SC9014 notice every project here shows).
