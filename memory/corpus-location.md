---
name: corpus-location
description: "The rc.17 sweep corpus was rebuilt 2026-10-25 at ../solid-checker-rc17-corpus/apps from fresh clones (23 apps, lutra-console gone); use the trimmed manifest copy; totals are not comparable with pre-rebuild sweeps"
metadata:
  node_type: memory
  type: project
  originSessionId: f7acefda-1e86-42dc-a3ef-0469297f2477
  modified: 2026-10-25T04:45:22.824Z
---

The original corpus lived on the owner's other machine. On 2026-10-25 it was rebuilt next to this checkout at `../solid-checker-rc17-corpus/apps` from the owner's list of 23 repos + commits (shallow clones), with every version-table Solid package written as an exact version in each `package.json`, `@solidjs/signals` 2.0.0-rc.17 pinned by override (npm `overrides`, bun `overrides`, yarn `resolutions`, pnpm `overrides` in the install root's `pnpm-workspace.yaml`), installs with lifecycle scripts disabled, bun run with `--minimum-release-age=0`, yarn with `YARN_ENABLE_GLOBAL_CACHE=0`, viviana-ui-web with `allowUnusedPatches: true`. All 34 manifest projects resolve solid-js, @solidjs/signals and @solidjs/web rc.17.

lutra-console's repo was deleted; since 2026-10-25 the repo manifest `scripts/corpus/rc17-manifest.json` itself carries repository/commit/installDir/packageManager per app, drops lutra-console and notes the rebuild, and `corpus-sweep.mjs` refuses a manifest without provenance (so the old /tmp copy no longer works; use the repo manifest). Build scripts and per-app logs: `/tmp/sc-corpus-build/`. Reports: `/tmp/sc-sweep/rc17-sweep-rebuilt-3.json` is the clean baseline (34 projects, 8,248 findings, 372 violations, 7,876 uncertifiable, no SC9014); rebuilt-4 = after ADR 0393 veto narrowing (7,728 uncertifiable, -148, none added); rebuilt-5 = after ADR 0394 spread declared keys (6,848 uncertifiable, -880, none added; release checker must be rebuilt after a producer protocol bump or every project fails the handshake); rebuilt-6 = ADR 0395/0396 first cut (no corpus change: misses were async `it` callbacks and test-only helpers); rebuilt-7 = ADR 0397 leaf callees (6,830 uncertifiable, -18); rebuilt-8 = ADR 0396 extension (async test callbacks, test-only helpers): 6,444 uncertifiable, -385 (kui 374); rebuilt-9 = ADR 0400 callback invocation roles: 6,052 uncertifiable, -392. Violations stayed at 372 throughout; nothing was ever added. Running total from baseline: -1,824 obligations.

**Why:** fresh installs resolve newer transitive dependencies, so this corpus is not byte-identical to the original.

**How to apply:** compare only sweeps made on this rebuilt corpus (baseline rebuilt-3); never against totals recorded in commit messages before 2026-10-25. Run with [[corpus-sweep-apps-root]]'s command and `--manifest scripts/corpus/rc17-manifest.json`.
