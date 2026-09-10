# Takeover of the additional coverage work

The main worktree contains the other agent's ADRs 0069–0072 and their
implementation: positional input-origin premises (protocol 43), independent
selection across prepared graph cases, graph preparation failure isolation,
and automatic recovery routing from the dependency-refusal census. These
changes extend ADR 0068's fallback work and have been preserved. No new commit
exists; HEAD remains `6b687543`.

The latest completed report found is
`rust/target/ecosystem-investigations/2026-09-08-receipt-composition-full-v2.json`,
finished 2026-09-08 at 12:38:16 JST, 817.656 seconds. Its documented aggregate
is 324 complete / 62 partial / 23 refused / 9 not advanced, 1,152 certified
entrypoint names and 371 rows with root coverage. The Type Facts source
manifest matches the installed producer stamp:
`9a9039269c78e4914d6ec68aa80e28f5a39eecab8927757bb2407575c3dc4767`.

The report and its preceding prepared-selection baseline have no retained
artifact references. An attempted exact catalog audit therefore stops at the
missing `retainedArtifacts`, rather than treating counts as exact case-set
preservation or looking through arbitrary leftover catalogs. The other agent's
aggregate comparison is not an artifact-bound preservation proof.

Fresh full `make verify` on the current inherited tree passed with actual exit
0, `TOTAL 82.46s`, and no `FAILED during step` marker. The log is
`/private/tmp/handoff-verify.log`. The older fallback verification also finished
successfully, but predates the inherited changes and is historical evidence.

A fresh 418-probe measurement completed through
`/private/tmp/run-handoff-retained-full.mjs`, with `--keep-temp`, the latest
report's exact reviewed recovery IDs, the current census-driven routing,
concurrency 4, certification concurrency 1 and timeout 900 seconds. Its target
is `2026-09-08-handoff-retained-full.json`; log
`/private/tmp/handoff-retained-full.log`. It exited 0 at 14:16:21 JST after
2,137.323 seconds. Its executable certification path and binaries remained
fixed throughout the run. The exact comparison script is
`/private/tmp/measure-handoff-retained-full.mjs`; it compares against the older
fully retained `initial-reads-full` report. It cannot reconstruct missing
historical catalogs from the later aggregate-only reports.

The completed census is **324 complete / 62 partial / 23 refused / 9 not
advanced**, matching the later inherited aggregate. Compared with the older
retained baseline (318 / 61 / 30 / 9), six Marker/I18n rows move from refused
to complete and Kobalte Utils moves from refused to partial. Published artifact
selections increase from 1,459 to 1,492. The exact inventory is
[handoff-retained-full-measurement](2026-09-08-handoff-retained-full-measurement.json).

The strict identity comparison flags two replaced closure hashes, both for
Solid 2's `./refresh`. These are not missing runtime entrypoints: runtime and
declaration bytes, export branches, closure file entries and export claims
remain identical. The graph replaces the unaccepted `solid-js` dependency
hazard with an exact authenticated dependency selection. The
[closure-transition evidence](2026-09-08-refresh-closure-transition.json)
records both identities and receipt/trust roots. Do not describe this as exact
closure-identity preservation or as two additional executable entrypoints.

The [full claim comparison](2026-09-08-handoff-all-claim-preservation.json)
checks every one of the 1,459 previously accepted runtime/declaration/branch
selections: none is missing and all retain identical export claims. The two
closure-context transitions remain explicit. This is 33 additional executable
artifact selections, including four root branches for Solid 2, with no change
to the coverage metric or its wildcard, scoped and inapplicable distinctions.

Next investigation: the measured 32-case recovery ceiling blocks larger sets,
but raising it alone already exceeded the 4,096 MiB process-tree limit and
lost existing coverage. A solution must bound memory while preserving exact
case, importer, dependency-receipt and trust identities and final publication.
The positional whole-root gap and duplicate compiler contexts also remain
open. ADR 0012's self-package dependency binding must not be bypassed. The
coverage objective remains active; no new certification is claimed by this
takeover audit.
