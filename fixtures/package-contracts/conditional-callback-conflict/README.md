# Two conditional targets that prove different callback executions

`schedule(callback)` calls `callback()` in the `development` target and
`queueMicrotask(callback)` in `default`. Both targets are analyzed, so both
answers are proven: `inline` in one environment, `deferred` in the other.

Per-target the contract is right, and `variants` carries each branch with its
`precedence` — the export map is ordered and resolved first-match-wins, so a
consumer selecting `development` resolves the `inline` branch and everything
else falls through. That much this fixture pinned already, through
`package_generator_orders_overlapping_conditional_callback_semantics` in
`rust/crates/solid-facts-backend/tests/contracts_process.rs`.

What it did **not** pin is the environment-unaware base, and the base was
wrong. `mergeSummaries` unioned the two targets' callback rows, so it carried

```json
"callbacks": [
  { "parameter": 0, "execution": "deferred" },
  { "parameter": 0, "execution": "inline" }
]
```

— two mutually exclusive claims about one parameter, and its comparator broke
ties on `execution` precisely because that was expected. Schema v1 has one
execution axis per parameter and a selected target has one behavior, so a
consumer reading the base picked one of two rows at random and had no way to
know which. `returns` and `asyncBehavior` had been given the sentinel for this
exact shape (`conditional-returns-divergence`); `callbacks` had not.

The base is now `{"status": "unknown"}`, the exact per-branch claims stay in
`variants`, and the review plan's `unknown-sentinel` item for
`.:schedule: callbacks` names both branches and the contradiction under
`because.divergences`.

The intra-target twin is `multi-role-callback-parameter`: one parameter invoked
twice *within one target*. The Rust sentinel that closes that one runs per
analyzed target and structurally cannot see this, which is why both pins exist.

`expected.json` puts this fixture in `scripts/contract-corpus.mjs`, so a
regression of the union specifically fails a gate rather than only the
variants-shaped assertion in the process test.
