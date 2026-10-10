# release-triple-flush-rc3

`flush()` inside action bodies on the audited `2.0.0-rc.3` triple, where it
does not throw. `@solidjs/signals` gained the `FLUSH_IN_ACTION` guard in
`2.0.0-rc.8`; rc.3's `flush` drains inside an action step as it does anywhere
else, in dev and production (the rc.1-rc.8 release review § 3, probe R:
`resolved` on rc.0-rc.7). The typings are identical on both releases, so
`tsc --noEmit` (5.9.3, `strict`, bundler resolution) is clean on `App.ts`
against this fixture's stub and against the published rc.3 install, and the
difference is only in the runtime.

The throw is keyed on the resolved `@solidjs/signals`, so the same source is
reported on rc.8 (`release-triple-flush-rc8`) and rc.9
(`rc9-flush-in-action`). Expected here: **no finding**, and no SC9014 (the rc.3
triple is audited).

`node_modules/` holds the three package manifests at `2.0.0-rc.3`.
