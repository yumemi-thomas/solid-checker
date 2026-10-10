# Stable members, deeper return paths and source revisions

The prototype now resolves exact local object methods, follows longer helper
chains, and refuses observations from another consumer source revision. Both
previous misses are covered: the thirty-row caller population moves to
**10/10 targets**, and the twenty-two-row return-path population moves to
**6/6**. Their **32 controls remain quiet and correct**.

This exploration adds **54 new consumers** in two populations. Four published
TypeScript errors are excluded before execution. No historical sources,
labels, frozen detectors or accepted observations were changed.

## Results

| Population and detector | Matching targets | Quiet, correct controls | Type exclusions |
| --- | --- | --- | --- |
| Earlier thirty callers, adapted v12 replay | 10/10 | 18/18 | 2 |
| Earlier twenty-two return paths, adapted v12 replay | 6/6 | 14/14 | 2 |
| Thirty-four new consumers, v12 sealed before authoring | 10/12 | 20/20 | 2 |
| Same thirty-four consumers, adapted v13 | 11/12 | 20/20 | 2 |
| Twenty further consumers, v13 sealed before authoring | 5/6 | 12/12 | 2 |
| Earlier twenty-nine challenge consumers, v13 replay | 10/10 | 15/17 | 2 |

The latest detector matches **16/18 targets** in the new populations, with
**32/32 quiet and correct controls**. The first population's v13 result is
adapted; the second is a fresh transfer. Every new target demonstrates stale
displayed output after a real update. There are no page errors, harness
failures, unrelated-feedback successes or failed controls in the new runs.

The original heldout result remains **18/18 target feedback**. Its twenty
controls stay quiet, but the existing pagination package defect still fails
one control's behavior check. The two earlier intentional-snapshot hints
remain unwanted feedback. They are retained in the score.

## Exact local member resolution

`local-call-targets-v1.mjs` resolves own properties of a local object literal.
It supports shorthand fields, assigned fields, inline arrows, inline methods,
immutable receiver aliases, extracted function aliases, literal keys and
immutable literal-key bindings.

The member must resolve to one exact declaration belonging to that object.
TypeScript can expose a transient member symbol distinct from the definition's
symbol. The model joins those symbols through their identical resolved
declaration node. It never falls back to a property name or a nearby span.

The receiver also needs a mutation and escape check. The model rejects writes
to a method, writes through receiver aliases, unknown uses of the object,
accessors, spreads, computed property definitions and receiver-dependent
`this` behavior. An observed package read cannot turn a replaceable member
into an exact static call target.

The first fresh population tests these distinctions against actual browser
behavior. Method replacement directly, through an alias and through an
escaped object produces the intended constant 9 and stays quiet. Explicit
snapshots, ignored arguments and deferred live reads also stay quiet.

## Nested object literals

The first transfer exposes an additional miss at `holder.api.read()`.
`local-call-targets-v2.mjs` follows exact own declarations through nested object
literals and tracks aliases to their child objects. Mutation checks cover the
root, each child, and every visible alias.

The adapted result closes that target and leaves the escaped receiver open.
The next population tests constant computed paths, a seven-helper path through
a nested method, a resize child alias with a parameter, a nested map arrow,
and an extracted nested set method. All five get matching feedback with the
detector sealed before their authoring.

New controls replace the entire child object, replace its method through an
alias, or pass the child to a function that replaces its method. They all
remain quiet and display 9. Nested accessor snapshots, discarded return
values and explicit untracking of a nested chain also remain quiet.

## Longer helper paths

The four-helper limit is replaced by an iterative traversal of the finite
local helper graph. Revisiting a helper refuses the path. Each helper still
needs one admitted return expression, and the observed stack must match every
return edge plus the setup call.

This closes the earlier five-helper miss. Fresh real consumers exercise six
helpers for map size and twelve for set size. Both get matching feedback;
their tracked controls display the updated values without feedback. An
independent audit reconstructs each exact declaration and return/call span.

The runtime collector still has a bounded stack. A truncated stack or missing
frame refuses the path. Removing the source graph's artificial depth limit
does not establish coverage of arbitrarily deep or recursive execution.

## Refusing stale observations

An additional regression test exposes an earlier gap: generic getter hints
authenticate package source bytes but do not authenticate the consumer digest.
Caller-path hints already check that digest. V12 applies the consumer check
before every observed getter channel.

Mapped frames must belong to the current source bytes, and the first mapped
consumer frame must agree with the recorded primary location. Missing mapped
source digests are refused by default. An explicit legacy replay mode exists
only for older observations whose source and profile inputs the offline study
runner authenticates separately.

The unchanged original mouse pair is rerun with actual mapped frames. An
experiment then compiles a virtual next revision with an appended comment,
without editing either consumer file or its recorded observation. Both
revisions pass published typing validation. The current-source target retains
its hint. V11 incorrectly retains that hint for the changed revision; V12
refuses it. The live control remains quiet in both revisions.

The first epoch-study script incorrectly required the tracked control to have
a skipped-tracking guard. It refused the run and wrote no report. The retained
v2 script permits that control's empty observations and keeps the target's
guard requirement. This is a source/observation boundary test; it is not a
complete hot-reload or editor integration test.

## Remaining open cases

- **Escaped receiver:** the new `h.holder = holder` target still misses.
  Unknown external code can replace a member, so the current model refuses it.
- **Borrowed child object:** `const api = { read }; const holder = { api }`
  still misses. The nested model follows inline object literals; it does not
  yet prove the alias graph for objects stored in other objects. Its live
  control passes and the target remains in the denominator.
- **Other unsupported paths:** cross-file helpers, async returns, branching
  or multiple returns, default/rest/destructured parameters, mutable function
  aliases, getters/setters, spreads, receiver-dependent methods and unknown
  returned calls remain open.
- **Intent and execution coverage:** the two earlier implicit intentional
  snapshots remain noisy. Observations cover executed paths; absence of a
  guard proves no defect. Snapshot feedback remains informational with
  `category: intent-open`, `certification: false` and `staticDispatch: open`
  for the external package behavior.

The changes share consumer-flow capabilities across packages. They add no
package/export recipes, certified contracts or label-based detection. Further
useful work is an exact object-alias graph, cross-file return paths, and live
integration of the source-revision boundary.

## Packages, verification and cost

Both fresh populations use the retained mouse `4.0.0-next.3`,
resize-observer `4.0.0-next.3`, map `1.0.0-next.2` and set `1.0.0-next.2`,
with Solid/signals/web `2.0.0-rc.9`. Each population pins 314 real declaration
files and the exact package closures. No package installation or network
lookup was required.

Published typings report TS2345 for a wrong method key, TS2339 for a missing
member, and TS2540 for readonly member replacement. Those four cases receive
no checker feedback and never execute in the browser.

- **162/162 prototype tests pass**, with no skips; **223 modules** pass syntax
  checks.
- `make verify-fast` passes producer freshness, Rust formatting and pinned
  workspace Clippy. Diff, schema and dialect validation pass.
- Independent validators authenticate source, labels, flows, typing
  exclusions, package closures and before/after browser profiles. They mark
  adapted replays separately from both fresh detector transfers.
- Independent auditors reconstruct **32 distinct accepted helper/member
  witnesses** across the earlier caller/path populations and the two new
  populations. Member audits check exact declarations and mutation/escape
  boundaries; they do not import the detector's resolver.
- The original 162-file freeze and all v4, v5, v7, v9, v10, v11, v12 and v13
  detector freezes remain unchanged.

The thirty-four-row browser run takes 35.24 seconds; the twenty-row run takes
21.16 seconds. The two-row source-digest recording takes 2.90 seconds.
Authenticated combined replays take roughly 7–11 seconds, including the
original forty-row population. These are local experiment timings rather than
editor latency or cold analysis benchmarks.

Full `make verify`, production coverage/ownership gates, contract conformance
and certification gates are deferred for this isolated prototype. No Rust
rule, package contract, public manifest or finding snapshot changes. Generated
research evidence is confined to `rust/target/snapshot-*`.

## Reproduction

The new detector modules are `snapshot-feedback-v12.mjs` and
`snapshot-feedback-v13.mjs`; each uses a versioned local call-target module.
Population modules are `snapshot-member-cases-v1.mjs` and
`snapshot-nested-member-cases-v1.mjs`. Earlier modules are preserved.

Use fresh output paths:

```sh
node benchmarks/reviewed-package-models/snapshot-refinement-study-v13.mjs \
  rust/target/family-holdout-validation.json \
  rust/target/snapshot-original-guard-browser-v2/browser/results.json \
  rust/target/snapshot-nested-member-new-study-v13.json \
  rust/target/snapshot-nested-member-browser-v1/browser/results.json

node benchmarks/reviewed-package-models/snapshot-challenge-validation-v4.mjs \
  rust/target/snapshot-nested-member-preflight-v1/population.json \
  rust/target/snapshot-nested-member-browser-v1/browser/results.json \
  rust/target/snapshot-nested-member-new-study-v13.json \
  rust/target/snapshot-nested-member-new-validation-v13.json

node benchmarks/reviewed-package-models/snapshot-caller-audit-v4.mjs \
  rust/target/snapshot-nested-member-new-study-v13.json \
  rust/target/snapshot-nested-member-browser-v1/browser/results.json \
  rust/target/snapshot-nested-member-new-audit-v13.json

node benchmarks/reviewed-package-models/snapshot-source-epoch-study-v2.mjs \
  rust/target/snapshot-member-original-caller-browser-v1/browser/results.json \
  rust/target/snapshot-member-new-source-epoch-v12.json
```

Evidence prefixes under `rust/target/` are `snapshot-member-*` and
`snapshot-nested-member-*`. They retain populations, observations, adapted
and fresh studies, validations, independent audits, and test/check logs.
All artifacts are research observations with no certification authority.
