# RESUME: ADR 1174 (W9, paused 2026-11-21 ~21:40)

Branch `claude/refresh-prop-memo-1174` (base fdc28bae3). This note is not for merge; delete it before the final commit.

**Done.**
- Step 0, main fdc28bae3: on mx-src RWOS (memo, effect and render computes, prop or context sources; 2,394 programs) 348 agree and 2,046 recall-gap. Every refresh program is U.
- Cause: in `static_api.rs`, a refresh whose target is not a source symbol takes ADR 0509's branch, which pushes the write with `Unknown` although its role (TrackedJsx) reports.
- Fix: `props_refresh_source` and `props_target_slot` resolve `props.name` (and, untested, a destructured slot) through `passed_on_every_site`, requiring one exact refreshable source. The write takes its role and the source's `ownedWrite`. `late_chunks.rs` gains the recorded reads `props_passed_on_every_site` and `props_attachments` (DiscoveryRead::PropsAttachments).
- At 3a264293f:
  - Release build in the VM, then Chrome on the fixture `refresh-prop-target`: the V lines 9 and 24 are covered-violation, the handler is quiet, the twins are covered-uncertifiable.
  - Probe, main fdc28bae3 vs 3a264293f: mx-src 90 recall-gap→agree (refresh with an `s-prop` source, every binding and layer), nothing else moved; lg-src no change.

**Uncommitted at pause → committed as WIP, NOT BUILT.** The destructured-slot path (`props_target_slot`) and the fixture case `DestructuredParent`.

**Next.**
1. Build (host debug), run the fixture; Chrome on the fixture; the probe against the m74 rows (VM: `~/EDF/tmp.noindex/w9/vm/job74.sh <sha>`, ids in `~/EDF/tmp.noindex/w9/sel3`).
2. Still U: `prop-merge`/`prop-omit` views, `prop-spread`, context sources (`refresh(ctx.m1)`, the context-provided machinery of ADR 1147), and signal/store setters through layers and bindings.
3. Gates: `make coverage` with SOLID_CHECKER_GATE_CONCURRENCY=4; guard drops rows (`node scripts/guard/count.mjs --update --adr 1174 --proof rust/crates/solid-reactive-ir/src/static_api.rs::props_refresh_source --fixtures reactive-ir/refresh-prop-target`), the ownership gate, verify-delta, a decisions.md entry, and the rule page `docs/rules/no-write-in-pure-computation.md`.
