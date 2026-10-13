# Late callback lineage and zero-noise assertion feedback

## Outcome

The research prototype recovers all four previously missed queue targets.
Automatic source filtering also removes two noisy named-object controls. With
explicit primary-value assertions and measured isolated captures, the selected
guidance reaches all 32 targets and emits no actionable note for 38 controls.

| Authored population | Automatic target hints | Automatic noisy controls | Assertion-selected target notes | Assertion-selected noisy controls |
| --- | ---: | ---: | ---: | ---: |
| Original 58 | 20/24 → 24/24 | 10/34 → 8/34 | 24/24 | 0/34 |
| Fresh 12 | 8/8 | 2/4 | 8/8 | 0/4 |
| Combined 70 | 32/32 | 10/38 | 32/32 | 0/38 |

Zero noise here means **no actionable informational suggestion on these controls
when selection uses their explicit supplied assertion**. Four original control
replays still carry open results: two change another visible output and two
break their previously passing primary assertion. Those results remain available
for investigation and are not admitted as suggestions.

These are correlated authored serial/concurrent consumers of
`@solid-primitives/queue@1.0.0-next.3` with Solid, signals and web `2.0.0-rc.9`.
They do not estimate accuracy across real applications or other packages. All
observations and suggestions remain informational, `authority: false` and
`certification: false`. Production behavior is unchanged.

## Recovering the missed targets

The earlier runtime discarded a child read if the enclosing async body had
already returned. `native-read-runtime-v12.mjs` now retains the exact chain of
normally completed enclosing bodies. A late child can publish that lineage to
the original registered callback. Failed, incomplete, inconsistent and
over-budget chains remain refused. Traversal is bounded at 64 bodies.

The second gap was a synchronous callback created inside an enrolled async
helper and later invoked by a Promise reaction. Source model V3 records the
exact lexical function, enclosing helper and creation-call spans through
TypeScript declarations. Transform V10 enrolls that callback and the runtime
records its actual entry. The original entire call, receiver, arguments, return
and error behavior remain intact. No method-name assumption establishes Promise
dispatch or scheduling, and no additional Promise reaction is attached.

The first browser trial did not validate the expression-bodied callback entry:
its generated capture statement lacked an original source anchor. The final
transform anchors it to the original callback body, with three focused mapping
regressions and actual browser/source-map audits. The failed trial remains saved
and is excluded from successful detection counts.

Together these changes recover adopted-child and reaction-read targets in both
queue modes. The fresh challenge also exercises named reactions and three-body
lineage. A normal body return establishes source provenance; Promise settlement,
fulfillment contents and causal result flow remain unproved.

## Removing named-object noise

`constant-returned-data-v2.mjs` extends the earlier constant own-field model to
an exact local `const` whose initializer is an object literal, including
transparent TypeScript wrappers. Every reference to the same declared symbol
must be its declaration or a direct return in the same function.

Mutation, aliases, shorthand escapes, nested captures and passing the object
elsewhere refuse this extension. The consumer must still use only the admitted
constant primitive own fields. Identity use remains open. The independent data
audit reconstructs declarations, references and normal-return outcomes without
importing the selector.

This quiets two original named allocations and two fresh wrapped named
allocations. It does not prove Promise fulfillment, absence of later mutation,
absence of effects or application correctness.

## Why automatic zero noise needs another premise

The original population contains two target/control pairs with identical source,
helper source, published typing, measured values, visible text, errors and raw
feedback. Each pair differs only in its supplied expectation: the target wants
`2`, while the control accepts `1`. The measured original result is `1`.

`noise-zero-intent-boundary-v1.json` records the exact hashes and equal behavior.
A selector using only the equal source/runtime inputs must treat both members
the same. Detecting both targets therefore produces at least two noisy control
hints without an additional intent premise. The case label is only a scoring
label and is never an admissible selection fact.

The test-assisted channel supplies that premise as an explicit assertion.
`capture-replay-plan-v2.mjs` proposes an exact source edit without receiving the
expected value or target/control role. It handles lexical child links and
getters callable with no required arguments, including optional/default
parameters and object results. Independent audit V3 reconstructs the binding
route and edit. Generated replay cases have no desired value and no target role.

The existing assertion selector then admits guidance only when the supplied
primary assertion fails before the edit and passes in the measured replay.
Already-passing assertions stay quiet. Missing assertions, refused source
routes and assertions that still fail remain open. The source planner is not
given an answer to manufacture during execution.

## Replay limitations are visible

Fifty-eight isolated proposals were measured: 46 from the original population
and 12 from the fresh challenge. All 32 target assertions pass after capture.
Twelve original controls had no source proposal and already passed their
assertion; the other 26 controls receive no actionable assertion-selected note.

Captures change task counts in 56/58 replays and then-getter counts in two.
Four controls change visible output. In two identity controls, capture changes
the primary value from `1` to `2` and breaks a previously passing assertion.
These observations forbid calling the proposals safe autofixes. Other
assertions, effects, input values, executions and developer intent remain open.

The selected note says only that an isolated capture satisfied the supplied
primary-value assertion under the recorded interaction. It includes source
hashes, before/after values and task counts. A production system would need an
application's actual failing test and a broader regression run before presenting
a stronger repair claim.

## Freezes and validation

The earlier handoff's 588 code/input pins remain unchanged. The first new profile
freeze preserves 599 JavaScript files and precedes the initial fresh case forms.
The final profile freeze preserves 604 JavaScript files and precedes fresh V2's
renamed bindings and optional getter. Fresh V1 forms were authored but not
executed; fresh V2 is derived from those forms, so the challenge is correlated.
The original 58 are adapted cases. Generated captures are measured proposals,
not independently authored held-out applications.

Independent browser audits validate current source revisions, helper/callback
declarations, original entry/read frames, parent links, constant-data facts,
retention, typing exclusions and unchanged instrumentation behavior. All 130
plain comparisons pass: 70 original applications, 58 captures and two typing
controls. Capture and assertion audits independently reconstruct their narrower
claims. Three altered reports are refused: a changed lexical parent, a changed
named declaration hash and an invented proved-settlement claim.

Focused checks pass without skips:

| Check | Passing tests |
| --- | ---: |
| Body returns, adoption, timing and bounds | 21 |
| Late children and lexical callbacks | 8 |
| Constant returned data and exact named bindings | 42 |
| Independent returned-data audit | 14 |
| Lexical callback entry mappings | 3 |
| Final projector retention and malformed evidence | 22 |
| Assertion selection and open outcomes | 8 |
| Total | 118 |

The actual TypeScript 5.9.3 CLI passes all 256 valid source files across the 128
original/replay applications against the retained published package typings.
The earlier two invalid controls retain real TS2345/TS2769 evidence; the final
browser/projector run produces no additional observation or note for them.
No fixture stub is used to justify a finding.

Universal fast checks pass: producer stamp, Rust formatting, pinned workspace
Clippy, schema parsing, dialect manifests and whitespace. Full production
coverage, ownership, certification, performance and release gates are deferred
because this slice changes only research JavaScript and documentation. No
production rule, fixture snapshot, contract, compiler pin or runtime artifact is
changed. Generated browser copies and reports live under ignored `rust/target`.

## Artifacts and reproduction

The combined report is `rust/target/noise-zero-combined-summary-v1.json`.
Its generator authenticates input and validator hashes, preserves prior profile
pins and recomputes target/control counts and assertion eligibility.

Use fresh output paths for every command. All paths below are under
`benchmarks/reviewed-package-models/` unless qualified:

1. `feedback-revision-browser-v31.mjs CASES FREEZE OUT BROWSER reads|plain`
   executes original cases with the final profile freeze.
2. `feedback-revision-audit-v18.mjs CASES READS PLAIN OUT` checks those runs.
3. `capture-replay-prepare-v3.mjs CASES READS OUT` prepares exact proposals.
   The capture case modules load those proposals without expected results.
4. Run the browser and independent browser audit on the generated capture
   cases. `capture-replay-audit-v3.mjs PREPARED CASES READS PLAIN AUDIT OUT`
   independently checks edits and measured behavior.
5. `replay-assertion-feedback-report-v1.mjs CAPTURE_AUDIT OUT` applies supplied
   primary assertions. `replay-assertion-audit-v1.mjs FEEDBACK OUT` audits them.
6. `noise-zero-summary-v1.mjs OUT` authenticates the named saved reports and
   reconciles the counts. `noise-zero-handoff-freeze-v1.json` seals the final
   code and report artifacts separately from the pre-challenge profile.

Automatic feedback still has eight original noisy controls: caught rejected
Promises, caught throwing then-getters, pending Promises and accepted identity
behavior. Two fresh constant-reaction controls also remain noisy. Callback
result flow through those reactions, unknown dispatch, Promise settlement,
mutation after return, other application assertions, unexecuted paths and
general package accuracy remain open. This experiment establishes a useful
assertion-assisted route to quiet guidance, not universal zero-noise warnings.
