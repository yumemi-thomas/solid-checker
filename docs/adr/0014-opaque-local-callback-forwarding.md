# 0014 — Unknown enclosing execution cannot inherit a helper's callback timing

Status: accepted and implemented; final verification recorded below
Date: 2026-09-04

## Decision before code

For a parameter forwarded to an analyzed local helper, require the enclosing
callback chain to be classified and to reach the function that declares the
parameter. An unclassified wrapper or a chain that stops inside another
callable makes the export's callback knowledge unknown. Only a direct-body
call with an empty chain may retain the helper's own timing without composition.
Known wrapper chains keep their existing composition and native proof demands.

The existing fallback treats an unclassified chain as an empty chain. That
confuses "nothing encloses this call" with "the enclosing execution is unknown"
and emits the local helper's same-stack callback claim about its caller. The
reduced `opaque-callback-chain` fixture proves the error without any Solid
dependency: `retain(() => invoke(callback))` never invokes the arrow, yet the
generator proposes the same operation as the direct `invoke(callback)` control.
A returned object carrying the arrow likewise gets an unsupported same-stack
claim. The generator must preserve uncertainty before demand planning, not ask
the verifier to accept an execution that is unproven.

## Measured baseline and alternatives

The prior Kobalte 0.9.2 graph stops at solid-js 1.9.14's createReaction:
`callback parameter has no exact direct-call or resolved-argument flow`, demand
`sha256:05596de8b9acb119628dd17fb20788932d6664ecff0c36c3fbdb3f6a710b143f`.
Its onInvalidate parameter is forwarded to local untrack inside a callback
stored by createComputation; invoking that computation needs additional
result/object flow evidence. A callable declaration alone cannot supply it.

Accepting every captured callback flow is rejected by the never-invoked
fixture. Treating the missing outer wrapper as transparent is the demonstrated
bug. Modelling stored computation objects and returned tracker invocation is
future work with its own exact flow and schedule evidence. Keeping the whole
graph refused is sound, but removing this generator-created false positive
claim lets independent claims proceed honestly.

The weaker proposal has an explicitly unknown callback domain; consumers gain
no positive or negative premise from it. Direct and proven composed claims
retain their witnesses. No scheduled failed demand is waived, no creates
closure or mandatory veto is dropped, and no native acceptance rule changes.
Harness/Node pins, exact artifact bytes and sandbox scheme 6 remain unchanged.

## Scope and required evidence

This decision covers analyzed local-helper forwarding. Other proposal paths,
including direct primitive-slot fallback, are not automatically changed by it.
Pin direct, opaque-wrapper and stored-arrow controls, and require the native
verifier to reject a transplanted direct claim. Review all corpus moves before
updating snapshots, then repeat the real offline graph and the same three-row
recipe-bearing benchmark. Report any subsequent refusal as remaining work,
not as a completed certification.

## Implementation and observed frontier

`forwarded_callback_ambient_execution` now requires a classified
`enclosing_callback_chain` whose outermost span reaches the parameter owner's
body. The existing unknown-forwarding obligation keeps the callback domain
open. No witness acceptance rule changes. The native fixture verifies the
ordinary proposal and rejects the direct claim transplanted onto either
`Opaque` or `Stored`, with `callback parameter has no exact direct-call or
resolved-argument flow`.

The offline Kobalte 0.9.2 graph advances past createReaction to indexArray in
solid-js 1.9.14. Its exact new refusal is:

```text
Type Facts demand sha256:0cc7f64b7816e102584b7c786aacd96b5364fc8a89f4090befe97c88f8e357b1 is locally open: operation-reachability (artifact-case:5d5e36f1a70c0df7d4fd05f5c56a1db35ee73b3c7399caa64ce19654a004f2e5:indexArray): callback parameter has no exact direct-call or resolved-argument flow
```

Both graph runs use 20 canonical nodes, 13 authenticated published artifacts,
25 acquisition units, 20 proposal generations and one native case-set batch.
Both assert zero registry-cache misses. In the demand-plan observations,
createReaction and startTransition lose callback-0's unsupported positive
proposal and expose an unresolved callback domain. This is proposal correction,
not certification of either callback's execution. The indexArray refusal is
still enforced, and no accepted Kobalte 0.9.2 graph is claimed.

The non-updating corpus gate first reported the new fixture's missing
snapshot. Inspection of all generated outputs found no changes to any of the
89 existing fixtures. The new fixture adds one artifact case, one positive
operation (`Direct`), 10 proposal-plan candidates and 25 unresolved claims.
Only the new main and proposal sidecar are generated; there is no refusal
sidecar to pin because its four disposition arrays are empty. Phase19's tracked
stable-main count increases from 180 to 181, and the directory is staged before
that gate. Creates candidates remain proposals subject to their existing
recipe requirement, not newly certified negative claims.

The retained graph audit files under the task's `scratchpad/probe-ts/` have
these SHA-256 digests:

| File | SHA-256 |
| --- | --- |
| `092-frontier/captured-read.audit.json` (before) | `7f1cfa8693474ee07e878ee3558d59025af54a79718df97e69d98a38be3d32b6` |
| `092-frontier/opaque-callback.audit.json` (after) | `8d76a7531d9b287cd74c6c92747aba2fe8c94cfd3dd92089242a268e1ac9ea8e` |

## Three-row comparison and verification

The baseline for this slice is our preceding recipe-bearing
`captured-member-verified.json`, SHA-256
`f9a52f23b1bf9ade7c56a559c6a5a4b40d679823e66ae161738d6735c69b27c6`.
It is the same three-row command and checked-in recipe corpus as this slice's
after run. `benchmarks/ecosystem/report.json` omits the corpus and is not used
as a comparison. The separate offline graph investigation above uses the
dependency-graph lane; the three-row measurement uses the runner's
reused-proposal lane. Advancing one does not imply that the other certified.

The after report is `opaque-callback-verified.json`, SHA-256
`7a7959e1d64c73a0ae04829898ef469ba9a119b1458a61c3822683c9edcf6997`.

| Row | Candidates / structurally loadable JS | Before | After |
| --- | --- | --- | --- |
| `@kobalte/utils@0.9.2\|solid1\|only` | 33 / 0 | noop gate incomplete; row refuses | Identical gate and refusal; no completed gates |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | 13 / 6 | Two clamp gates complete; 11 candidates withheld | Identical two closures, 11 withheld |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | 3 / 3 | Census refuses before gate execution | Identical refusal; all three gates unexecuted |

The 0.9.2 refusal remains exactly `mandatory probe gate
sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc
did not complete`. Its failed-row audit's zero withheld count is not evidence
that the other 32 candidates ran. The i18n demand remains
`sha256:275adf7fd0fc552a91c58cd454dd7a4a4db64bcb4151da1c454eeff9b80a3e43`,
`creates census refuses an uncensused invoking form:
property-access-unknown-accessor (SpreadAssignment)` at `dist/index.js:3471..3483`.

Totals are unchanged: **49 candidates, 9 structurally loadable JS candidates,
2 completed gates, 0 runtime contradictions, exportsProven 0**. Of the original
43 candidates, **0 become newly probeable in this slice**; their 40 TS-source
cases and 3 JS cases retain their prior disposition. The six additional alpha
JS candidates came from ADR 0010. The independent accessor/iteration census
restrictions remain unchanged. A runtime contradiction would be a successful
veto, but neither real completed gate produced one; forged-claim fixture
refusals are Type Facts verification tests, not runtime contradictions.

The two alpha accepted main/probe-root pairs remain byte-identical:

| Case | Accepted main SHA-256 | Probe gate root SHA-256 |
| --- | --- | --- |
| import | `ba248e10d7f21d3e403c5e9e016ccbd07981800726b73c0acfd1aa1ed606bc67` | `7f198a137e0a5cfdcf4b5eead8bd374b4d283b8cbefbe70a4081b38f7bf18278` |
| solid | `95cf4186f9b2110182908569e2cc79e7951883e942979e0ba11fa705b7085528` | `f3b2dcb43c6dde2199a9a0c3eca353e874c3be0588beeacb78d6fea7a7ea767f` |

Verification passed with one Cargo process at a time through the Makefile:

| Check | Result |
| --- | --- |
| Focused native regression | Direct verifies; both forged siblings refuse |
| Probe harness | 97 passed, certification pins required |
| Backend library / IR library | 369 / 234 passed |
| Armed contracts / diagnostics / dialects process suites | 7 / 15 / 37 passed |
| Contract corpus | 90 fixtures pass after reviewed new snapshots |
| Coverage | 94 projects, 546 findings; no finding moves |
| Ownership | 289 cases, 465 ledger rows, 0 pending |
| Vitest scripts, including phase19 | 153 passed; 181 tracked stable mains |
| CLI | 173 passed, including the type-check step |
| Formatting / workspace Clippy | Pass; `--all-targets -- -D warnings` |
| Schema / dialect manifests / diff checks | Pass |

The pinned debug binary was rebuilt in a separate Makefile invocation after
Clippy and before the final benchmark. No full ecosystem benchmark, ledger or
benchmark repin, `make verify`, commit or push was performed. The lead retains
the full handoff gate. Next work needs a reduced indexArray callback-flow case;
the refusal is evidence of missing proof, not a package defect.
