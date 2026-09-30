# Package-directed contract pilot

This offline experiment tests whether explicit proposals improve findings while
using the existing verifier. It does not add proof rules or publish into the
accepted tier. Read the [result](../../docs/package-contract-v2/phase22/2026-10-01-package-directed-pilot.md)
for measured limits, including the distinction between an authenticated partial
receipt and a complete package.

Prerequisites are the current pinned release checker and Type Facts producer,
retained rc.9 package installs from the last checkpoint, cached exact registry
archives, and the existing policy-2 issuer configurations. Missing inputs stop
the experiment. Use make targets if a binary needs building; the pilot never
builds or installs anything.

```sh
PATH=/Users/thomas/.bun/bin:$PATH \
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
SOLID_CHECKER_PROBE_NODE=/Users/thomas/.vite-plus/js_runtime/node/24.21.0/bin/node \
SOLID_CHECKER_REGISTRY_CACHE="$PWD/rust/target/registry-cache" \
SOLID_CHECKER_MATERIALIZED_STORE="$PWD/rust/target/materialized-store" \
bun benchmarks/package-directed-pilot/pilot.mjs \
  rust/target/primitives-checkpoint/run.json \
  rust/target/package-directed-pilot-fresh
```

Choose an unused output directory. Existing evidence is never removed. The
runner also creates isolated consumer source directories beside the retained
installs so exact package and dependency identities stay intact. Each trial
records its automatic proposal, authored proposal, audit, authenticated main,
receipt, trust configuration and consumer observations. `results.json` is the
combined observation record. Original proposal-plan sidecars describe generation
only; the native transaction derives fresh demands from the authored proposal.

The three positive package trials use browser conditions. The fourth is a
negative callback-enumeration control. Ten strict TypeScript checks precede
twenty consumer analyses; their expected findings are asserted. A successful
run establishes the recorded partial results, not full package certification.

Append `extended` after the output directory to compare listener owner bounds,
RAF ownership in none/browser/node, the corrected conditional RAF tuple return,
and a browser `memo.createPureReaction` replication control. This mode performs
eight trials, sixteen TypeScript checks and thirty-two consumer analyses.
The [extension result](../../docs/package-contract-v2/phase22/2026-10-01-package-directed-extension.md)
records the remaining proof gaps. To check a saved extended record without
repeating analysis:

```sh
bun benchmarks/package-directed-pilot/check-extended.mjs \
  rust/target/package-directed-extended/results.json
```
