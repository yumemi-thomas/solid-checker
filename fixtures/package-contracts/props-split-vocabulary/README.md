# A props split is a props split under either dialect's spelling

`splitProps` (1.x) and `omit` (2.0) are the same primitive: a props object plus
key lists in, property views out. Neither ever invokes an argument, so the
generator's callback inventory has a suppression for them — without it, an
artifact whose types are erased leaves every argument's callability unknown,
and the key list raises an unknown-contract-callback obligation that no
evidence can ever discharge.

That suppression was written as a comparison against `Primitive::SplitProps`,
which is **1.x's** spelling. It began life as the literal `"splitProps"` and
survived the dialect-seam extraction (`ecf6e0d8`) as a constant rather than as a
row, so it answered for one vocabulary and was silent for the other. This pair
is the differential:

| | `closed` |
| --- | --- |
| `props-split-vocabulary-v1` (`splitProps`) | `callbacks`, `reads` |
| `props-split-vocabulary` (`omit`), before the row | **`reads`** only |
| `props-split-vocabulary` (`omit`), after | `callbacks`, `reads` |

An open `callbacks` domain is not a cosmetic difference: it is exactly what a
consumer's open-claims gate reports, so every 2.0 package that split its props
published a domain it had no reason to leave open.

## The `creates` column is a third asymmetry, and it is not fixed here

`callbacks` is the claim under test. `creates` differs between the halves too,
and `expected-refusals.json` says why: the 2.0 half declines with
`kind: "dialect-silent"`, `callee: "omit"`. `Solid1x`'s negative-claim
authority carries an audited `splitProps` row citing exact byte ranges in
`solid-js@1.9.14`'s six bundles; `Solid2`'s table has rows for `createMemo`,
`createSignal`, `createStore`, `createOptimistic`, `createOptimisticStore`,
`flush`, `onSettled` and the rest — and none for `omit`.

So the same primitive is asymmetric in *two* tables, and only one of them is a
seam bug. A negative-claim row is an audit: it cites a file digest, a byte
range and a slice hash against a written audit section, and it cannot be added
by inspection. Recorded in `docs/precision-backlog.md` rather than guessed at;
this fixture is what will show it closing.

## Why the artifact is untyped

Deliberately. With `omit`'s declaration in view, `keys` is provably
`readonly (keyof T)[]` and not callable, so the obligation never arises and the
fixture would pin nothing. Erasing the types is what puts the call on the arm
the suppression guards — the same condition the suppression's own comment
names.

The dialect row itself (`Dialect::splits_props`) is pinned separately by
`each_dialect_names_its_own_props_split_and_tuple_returns` in `solid-dialect`;
this fixture pins what it *does*.
