# Package-directed contract pilot

The [active browser and host-misuse experiment](../../docs/package-contract-v2/phase22/2026-10-01-active-browser-and-host-misuse.md)
replays the automatic matrix after ADR 0176 and tests host-specific ledger
expectations. The automatic runner accepts an optional comma-separated list
of package names after its output directory for focused replays.

```sh
bun benchmarks/package-directed-pilot/compare-composition-replay.mjs \
  rust/target/package-composition-breadth-1/results.json \
  rust/target/package-composition-deferred-reads-1/results.json \
  rust/target/package-composition-deferred-reads-1/comparison-fresh.json
SOLID_CHECKER_PROBE_NODE=/Users/thomas/.vite-plus/js_runtime/node/24.21.0/bin/node \
bun benchmarks/package-directed-pilot/probe-active-listeners.mjs \
  rust/target/primitives-checkpoint/run-node.json \
  rust/target/package-composition-deferred-reads-1/results.json \
  rust/target/package-composition-active-listeners-fresh
```

With the pinned checker/producer/probe environment below,
`check-host-misuse-controls.mjs` takes the same three arguments as the listener
probe. It uses existing ledger source verbatim without changing expectations.
`diagnose-media-bounds.mjs` optionally takes `all` after its output directory
to remove all bounds in an authored diagnostic; default `target` removes only
the maker's shared summary. Neither observation grants proof authority.

This offline experiment tests whether explicit proposals improve findings while
using independently authenticated proposals. The first three modes use the
original verifier capabilities; `cover` and `bounds` test ADR 0173's bounded
owner proof. No mode publishes into the accepted tier. Read the [result](../../docs/package-contract-v2/phase22/2026-10-01-package-directed-pilot.md)
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

Append `completion` instead to compare the dispatcher's inert node return with
its browser refusal and a platform control using all 23 exports. Six published
TypeScript checks precede twelve consumer analyses. These are replication and
refusal controls, with no new complete package claimed. The
[completion result](../../docs/package-contract-v2/phase22/2026-10-01-package-directed-completion.md)
also records the nested-handler consumer limit and checkpoint-wide inventory.

```sh
bun benchmarks/package-directed-pilot/check-completion.mjs \
  rust/target/package-directed-completion/results.json
SOLID_CHECKER_PROBE_NODE=/Users/thomas/.vite-plus/js_runtime/node/24.21.0/bin/node \
bun benchmarks/package-directed-pilot/probe-dispatcher.mjs \
  rust/target/package-directed-completion
bun benchmarks/package-directed-pilot/inventory.mjs \
  rust/target/primitives-checkpoint \
  rust/target/package-directed-completion/domain-families-fresh.json
```

Runtime probes and the inventory refuse to overwrite earlier evidence. The
probes compare exact source hashes and resolution conditions; passing samples
confer no proof authority. Inventory counts reuse retained measurements and do
not predict the gain from a future proof rule.

Append `cover` for the listener's guaranteed registration across browser/node/
none, plus browser RAF/memo controls. Append `bounds` for attempts to strengthen
the ten remaining possible owner operations across lifecycle, permission,
sensors, timer and workers. See the
[owner-cover experiment](../../docs/package-contract-v2/phase22/2026-10-01-owner-cover-experiment.md).
The earlier reports remain historical measurements; the new proof changes the
default pilot's listener result to a browser owner violation when running the
prototype checker. That implementation lives on `codex/owner-cover-experiment`
in `.claude/worktrees/codex-owner-cover-experiment`; point
`SOLID_CHECKER_NATIVE_BIN` at its release binary. The main verifier is unchanged.

```sh
bun benchmarks/package-directed-pilot/check-cover.mjs \
  rust/target/package-directed-owner-cover-final/results.json
bun benchmarks/package-directed-pilot/check-cover.mjs \
  rust/target/package-directed-owner-bounds/results.json
```

`composition.mjs` records the whole timer package's node/browser baseline and
an exact-runtime counterexample to claiming that its captured callback result
is always plain. It uses existing forms only. The
[composition baseline and proposed boundary](../../docs/package-contract-v2/phase22/2026-10-01-composition-timer-baseline.md)
records the owner decision needed before implementation. Use the same binary
and cache environment above, with a fresh output directory:

```sh
bun benchmarks/package-directed-pilot/composition.mjs \
  rust/target/primitives-checkpoint/run-node.json \
  rust/target/package-composition-baseline-fresh
bun benchmarks/package-directed-pilot/check-composition.mjs \
  rust/target/package-composition-baseline/results.json
```

`composed-cursor.mjs` tests existing described-callable shapes across a local
helper and an authenticated dependency graph. It uses the isolated
`codex/composition-callables` checker and authors proposals for all six cursor
exports. See the [result and limits](../../docs/package-contract-v2/phase22/2026-10-01-composed-callables-experiment.md).
Keep the producer, probe Node and cache environment above, but point
`SOLID_CHECKER_NATIVE_BIN` at that worktree's release checker:

```sh
bun benchmarks/package-directed-pilot/composed-cursor.mjs \
  rust/target/primitives-checkpoint/run-node.json \
  rust/target/package-composition-cursor-fresh
bun benchmarks/package-directed-pilot/check-composed-cursor.mjs \
  rust/target/package-composition-cursor-final/results.json
```

Append `open-dependency` or `withheld-dependency` to the runner with a fresh
output directory for the two node refusal controls. The latter proves an
incorrect child return cannot survive as a parent guarantee. No mode updates
the committed tier or treats the runtime falsifier as proof authority.

Append `automatic` for the cursor trial using only native-generated proposals,
including dependencies. The historical consumer field `authored` names the
experiment receipt in this mode too; `automatic: true` identifies the mode.

`automatic-composition.mjs` extends that test across seven additional exact
published packages and both hosts. It hashes every generated graph input and
asserts it is unchanged after certification. See the
[breadth results and remaining walls](../../docs/package-contract-v2/phase22/2026-10-01-automatic-composition-breadth.md).
Use the same pinned worktree checker, producer, probe Node and offline caches:

```sh
bun benchmarks/package-directed-pilot/automatic-composition.mjs \
  rust/target/primitives-checkpoint/run-node.json \
  rust/target/package-composition-breadth-fresh
bun benchmarks/package-directed-pilot/probe-automatic-composition.mjs \
  rust/target/primitives-checkpoint/run-node.json \
  rust/target/package-composition-breadth-1/results.json \
  rust/target/package-composition-breadth-call-time-fresh
bun benchmarks/package-directed-pilot/check-automatic-composition.mjs \
  rust/target/package-composition-breadth-1/results.json \
  rust/target/package-composition-breadth-call-time-1/results.json
```

The separate probe runner repeats only runtime observations against unchanged
certification evidence. It installs guards after normal module import. Earlier
pre-import guard failures are retained harness defects, not semantic findings.

`diagnose-media-bounds.mjs` is a separate authored control that removes only
the media listener's accessor bounds and asks native certification to prove
the stronger proposal. It is excluded from the automatic-generation counts:

```sh
bun benchmarks/package-directed-pilot/diagnose-media-bounds.mjs \
  rust/target/package-composition-breadth-1/media-node/execution.json \
  rust/target/package-composition-media-bounds-fresh
```
