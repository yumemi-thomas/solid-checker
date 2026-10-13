# Package feedback through observed callers and local return paths

Recording actual callers closes the two earlier helper misses. Immutable
aliases, ordinary parameters and short local return chains also work without
package-specific contracts. This is evidence for a reusable feedback family;
it does not establish coverage of every package or every rule.

This exploration adds **52 consumers**, of which four have published TypeScript
errors and are excluded before execution. It also reruns the unchanged sixteen
earlier helper consumers under a new recorder. Historical sources, labels,
detectors and accepted observations remain unchanged.

## Results

| Population and detector | Matching targets | Quiet, correct controls | Type exclusions |
| --- | --- | --- | --- |
| Earlier sixteen helper consumers, v9 | 4/6 | 10/10 | 0 |
| Same consumers and new caller recorder, adapted v10 | 6/6 | 10/10 | 0 |
| Thirty new caller consumers, v10 sealed before authoring | 5/10 | 18/18 | 2 |
| Same caller consumers, adapted v11 | 9/10 | 18/18 | 2 |
| Twenty-two further consumers, v11 sealed before authoring | 5/6 | 14/14 | 2 |
| Earlier twenty-nine challenge consumers, v11 replay | 10/10 | 15/17 | 2 |

The final detector matches **14/16 targets** in the 52 new consumers and keeps
**32/32 controls** quiet and correct. The thirty-row result is adapted; the
twenty-two-row result is a fresh transfer. The original heldout population
still receives feedback on **18/18 targets**. Its twenty controls stay quiet,
but the existing pagination package defect keeps one control behavior failing.

Every new target displays stale output after a real mouse, resize or collection
update. There are no harness failures, unrelated-feedback successes or failed
controls in the new populations. The two earlier intentional-snapshot controls
remain noisy. All snapshot additions have severity `info`, category
`intent-open`, and `certification: false`.

## Why recording callers matters

The old recorder deduplicated a guard by its nearest consumer location. Two
calls to the same helper therefore became one observation, even if one call
was explicitly untracked and the other supplied a displayed value.

`guard-trace-runtime-v2.mjs` preserves distinct consumer caller stacks.
`browser-experiment-v2.mjs` maps every guard frame back through the actual
compiler source map and records the consumer source digest. The versioned
browser profile freezes its code before and after execution.

An independent parity check compares the sixteen old and new browser runs:
consumer bytes, labels, package closures, published typings, displayed behavior
and native feedback codes are identical. Only caller observations change:
eleven recorded tracking guards become twelve. The discarded and captured
calls in the multiple-caller target now have separate witnesses.

V10 joins a package guard, a pure local property return and an exact setup
call. Both the return frame and caller frame must map into their real source
spans with the expected source digest. Additional references and escaping a
helper do not prevent a hint at an observed exact direct call. An observation
from `untrack(read)` cannot authorize a different `read()` call.

## Following a short return path

The fresh thirty-row population found five misses in v10: nested transparent
callee wrappers, immutable aliases, helper parameters, forwarded returns and
member callers. V11 closes the first four.

It resolves immutable identifier aliases through exact TypeScript symbols.
Each admitted helper has one return expression, ordinary identifier parameters,
and synchronous execution. A return can be a pure property read or a direct
call to another exactly resolved local helper. The observed stack must match
the whole path, from the returned package read to the setup capture.

For example, the same rule handles:

```tsx
const leaf = () => position.x;
const selected = leaf;
function read() { return selected(); }
const frozen = read();
return <p>{frozen}</p>;
```

The detector requires actual frames at the package read, the returned
`selected()` call and the setup `read()` call. It does not treat a read inside
an unknown argument as evidence that the returned value retains that read.
Mapped call punctuation is accepted; positions inside arguments and type
arguments are rejected. Transparent parentheses and TypeScript wrappers are
preserved across every layer.

The fresh transfer exercises three helpers with aliases, a passed object,
wrapped return and setup calls, a resize helper with a parameter and alias,
and a set-size return chain. All five receive matching informational feedback.

Controls cover explicit untracking at inner and outer calls, discarded comma
returns, ignored arguments, branching, mutable aliases, deferred live reads,
different helpers and unknown calls in a returned member. They remain quiet
and display their intended values.

## Limits that remain visible

- **Member dispatch:** `holder.read()` is a demonstrated miss. The detector
  follows exact identifier targets; it does not infer a member's runtime
  behavior from a name or a containing type.
- **Depth:** the frozen v11 limit is four helpers. The five-helper target
  remains a miss. Its live control passes. Increasing the bound requires a
  new detector version and a fresh transfer test.
- **Other open local paths:** cross-file helpers, asynchronous returns,
  multiple return statements, default/rest/destructured parameters, mutable
  aliases, unknown returned calls and deferred value paths are unsupported.
- **Observations:** runtime hints cover executed paths. A missing observation
  proves no defect. New helper witnesses bind consumer and package bytes;
  historical nearest-frame observations are used only in authenticated offline
  replays. A live integration still needs source/build invalidation across all
  observed channels, including after hot reload.
- **Intent:** an implicit intentional snapshot can still look like a defect.
  The two earlier noisy controls stay in the score. Exact native `untrack`
  states snapshot intent; absent intent does not justify escalating a hint
  into a proven violation.

The installed packages are mouse `4.0.0-next.3`, resize-observer
`4.0.0-next.3`, map `1.0.0-next.2` and set `1.0.0-next.2`, against Solid,
signals and web `2.0.0-rc.9`. No package installation or registry lookup was
needed. Declaration pins cover 314 files in the caller population and 252 in
the local-path population.

Real published typings report TS2630 for function reassignment, TS2339 for
the absent mouse property, and TS2345 for an invalid helper key. Those
consumers are excluded before browser execution and receive no checker
feedback. An initial mutation regression expectation exposed TS2630; the
test now asserts TypeScript ownership instead of inventing a semantic rule.

## What this says about a scalable system

The useful unit is a behavior family plus exact consumer flow. Source-derived
package guards and footprints supply evidence about reads and lifetimes;
project symbols and observed caller paths connect that evidence to an app.
The detector contains no package/export recipes or target/control labels.

This supports investing in shared analysis of aliases, return paths and
execution context. Those capabilities benefit many packages at once. Native
diagnostics and proven static facts can remain errors or warnings; observed
snapshot intent stays informational. Certification is still needed when a
claim requires external behavior that these sources and observations do not
establish.

The next useful trials are exact member resolution, cross-file return paths,
source invalidation during hot reload, and developer review in ordinary apps.
Those trials should retain unsupported targets and intentional controls rather
than optimize only the matched-target count.

## Validation and cost

- **133/133 prototype tests** pass, with no skips; all **209 modules** pass
  syntax checks.
- `make verify-fast` passes producer freshness, Rust formatting and pinned
  workspace Clippy. The universal diff, schema and dialect checks pass.
- Independent scoring validates both fresh populations and identifies the
  thirty-row v11 replay as adapted. Sources, labels, flows, package closures,
  real declarations and browser profile bytes are authenticated.
- Independent audits reconstruct **20 distinct helper/path hints** across
  the sixteen-, thirty- and twenty-two-row populations. They check exact
  declarations, alias edges, return spans, recorded frames, source digests
  and recomputed package guard premises.
- The original 162-file freeze and the v4, v5, v7, v9, v10 and v11 source
  freezes remain unchanged.
- One early replay was rejected because its source changed while it ran. It
  produced no result artifact. The subsequent stable replay passed the same
  before/after source check; rejected work contributes no score.

The caller browser replay takes 17.89 seconds for sixteen consumers. The new
thirty- and twenty-two-row browser runs take 30.77 and 22.21 seconds. Combined
source/observed replays take roughly 8–10 seconds, including authentication and
the original forty-row replay. These are local experiment timings, not cold
analysis or editor-latency measurements.

Full `make verify`, production coverage/ownership gates, contract conformance
and certification gates are deferred because this change is confined to an
isolated research prototype. No Rust rules, public contracts, manifests or
finding snapshots change. Generated research evidence lives under
`rust/target/snapshot-*`.

## Retained evidence and reproduction

Detector freezes: `snapshot-v10-detector-freeze.json` and
`snapshot-v11-detector-freeze.json` under `rust/target/`.

Key evidence prefixes there are `snapshot-helper-caller-*`,
`snapshot-caller-*`, `snapshot-local-path-*` and
`snapshot-caller-old-challenges-*`. They retain preflight populations, browser
observations, studies, validators, parity checks and independent caller audits.
`snapshot-caller-tests.log` and `snapshot-caller-verify-fast.log` retain checks.

Use fresh output paths when replaying:

```sh
node benchmarks/reviewed-package-models/snapshot-refinement-study-v11.mjs \
  rust/target/family-holdout-validation.json \
  rust/target/snapshot-original-guard-browser-v2/browser/results.json \
  rust/target/snapshot-local-path-new-study-v11.json \
  rust/target/snapshot-local-path-browser-v1/browser/results.json

node benchmarks/reviewed-package-models/snapshot-challenge-validation-v4.mjs \
  rust/target/snapshot-local-path-preflight-v1/population.json \
  rust/target/snapshot-local-path-browser-v1/browser/results.json \
  rust/target/snapshot-local-path-new-study-v11.json \
  rust/target/snapshot-local-path-new-validation-v11.json

node benchmarks/reviewed-package-models/snapshot-caller-audit-v2.mjs \
  rust/target/snapshot-local-path-new-study-v11.json \
  rust/target/snapshot-local-path-browser-v1/browser/results.json \
  rust/target/snapshot-local-path-new-audit-v11.json
```

These commands require the retained package installs and original authenticated
observations. They create research evidence and grant no certification authority.
