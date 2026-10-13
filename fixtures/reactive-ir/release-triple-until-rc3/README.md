# release-triple-until-rc3

`until` on the rc.3 triple, where it does not exist. The rc.1-rc.8 release
review (`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`
§ 6) measured SC2005 `until-in-tracked-scope` firing on this source on the rc.3
and rc.4 triples. `until` arrives in `@solidjs/signals@2.0.0-rc.5`, re-exported
from the `solid-js` root in the same release, so on rc.3:

- `tsc --noEmit` (5.9.3, `strict`, bundler resolution) reports
  `TS2305: Module '"solid-js"' has no exported member 'until'` on the import,
  against this fixture's stub (`solid-js.d.ts`, which declares no `until`) and
  against the real rc.3 triple;
- neither runtime has the export, so the dev guard SC2005 states
  ("Cannot call until inside a reactive scope") cannot happen.

The vocabulary knows `until` only where both `solid-js` and `@solidjs/signals`
resolve to rc.5 or later. Expected: **no finding**, and no SC9014 (the rc.3
triple is audited). The same `App.tsx` over the real rc.5 triple is tsc-clean
and reports SC2005 at the call, as `rc9-until-scope` does on rc.9.

`node_modules/` holds the three package manifests at `2.0.0-rc.3`, which is
what dialect detection reads.
