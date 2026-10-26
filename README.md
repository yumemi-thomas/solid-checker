# Handoff 2026-10-26 (not code; delete this branch once fetched)

Working files for PR #116 that lived outside the repo on the previous machine.
`tar xzf handoff-2026-10-26.tgz` gives:

- `corpus-build/`: scripts and pins that rebuilt the rc.17 corpus (`apps.json`, `versions.json`, `build.mjs`, ...).
- `sweeps/`: `aggregate.mjs`, `families.mjs`, sweep reports rebuilt-3 (baseline), rebuilt-9, soundness.
- `unc-research/REPORT.md`: root causes and ranked slices for the uncertifiable follow-up.
- `e2-design/NOTES.md`, `probes/REPORT.md`, `upstream-spec/` (eslint-plugin-solid v0.18.1 specs).
- `memory/`: the agent's project notes (toolchain, corpus, parity policy, series state).

## Update 2026-10-26 evening: `handoff-2026-10-26b.tgz`

For the reproduction-set follow-up (PR #116 at 8a2e977db): `REVIEW-AND-FIXES.md`
(independent review verdict and the fixes still to do), `corpus-freeze/`
(restore the exact rc.17 corpus: `node restore.mjs --apps-root ../solid-checker-rc17-corpus/apps`),
`sweeps/` (head, kui-closed, no-veto sweep reports), `classified/`, `meta/`,
`repro-review/` (reviewer scratch), `wip-loop-await-classifier.patch` (untested).
`memory/` holds the agent's latest notes (the ~/EDF, TMPDIR and Defender notes
apply to the old machine only).
