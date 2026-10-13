# implementation-census-callback-lower-bound

The tracer for ADR 0159
(`docs/adr/0159-a-strict-reachability-floor-is-a-lower-bound.md`). Every export
calls its callback directly in its own body, so the generator proposes, and
ADR 0100's census confirms, one `callbacks` item from parameter 0 with the
count `{scope: call, min: 0, max: many}` (`expected.json`).

The certifier's side is
`a_callback_claimed_to_run_at_least_once_needs_an_unconditional_call` in
`rust/crates/solid-facts-backend/src/contract_certification.rs`, which plans
the same package three times with the item's count replaced:

| count | certifies | why |
| --- | --- | --- |
| `min: 0` (as generated) | all seven asserted exports | each body may call the callback |
| `min: 1` | none, `always` included | the operation-cardinality census proves `0..many` and nothing tighter |
| unstated | `always`, `afterThrow` | the strict floor now asks for the producer's `unconditional`; `guarded`, `shortCircuit`, `chosen` and `early` certified before ADR 0159 on the optimistic `reach`, and `looped` never did |

`optional` (`callback?.()`) is bound but not asserted: its row is here to
keep the optional-call shape in the generated document.
