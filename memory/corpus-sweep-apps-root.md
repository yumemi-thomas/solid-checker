---
name: corpus-sweep-apps-root
description: Always pass --apps-root ../solid-checker-rc17-corpus/apps (and the rc17 manifest) to scripts/corpus-sweep.mjs; the owner says its default points at the rc13 corpus
metadata:
  node_type: memory
  type: feedback
  originSessionId: f7acefda-1e86-42dc-a3ef-0469297f2477
  modified: 2026-10-25T03:47:25.250Z
---

Owner instruction (2026-10-25): when running `scripts/corpus-sweep.mjs`, always pass `--apps-root ../solid-checker-rc17-corpus/apps` explicitly, together with `--manifest scripts/corpus/rc17-manifest.json`, because the script's default has pointed at the rc13 corpus.

**Why:** a sweep over the wrong corpus produces counts that are not comparable with the rc.17 baseline, and the error is silent if both directories exist.

**How to apply:** full invocation: `node scripts/corpus-sweep.mjs --manifest scripts/corpus/rc17-manifest.json --apps-root ../solid-checker-rc17-corpus/apps --checker rust/target/release/solid-checker-rust --typefacts bin/solid-typefacts --out <report.json>`; pass the previous report as `--order-from`. See [[corpus-location]] for where the corpus is.
