# A package callback run as an owned computation

Pins ADR 0183. An accepted contract can state that an export runs a callback
argument on every call, during the call, tracked, under a children-capable
owner the call creates. That is a memo or effect compute. A signal write
directly in a function literal at that argument then throws
`REACTIVE_WRITE_IN_OWNED_SCOPE`, as in a `createMemo` compute, wherever the
export is called.

| Case | Finding | Why |
| --- | --- | --- |
| `derive(() => setCount(1))` | `SC2001 reactive-write-in-owned-scope` violation | memo compute, `min: 1` |
| `watch(() => { setCount(3); … }, …)` | violation | effect compute, `min: 1` |
| `watch(…, () => setCount(4))` | none | the effect function is queued |
| `deriveMaybe(flag, () => setCount(5))` | `SC9005` uncertifiable | the export only may run it (`min: 0`) |
| `deriveWrapped(() => setCount(6))` | `SC9005` uncertifiable | no owner claim |
| `derive(() => () => setCount(7))` | none | the compute only returns the closure |
| `derive(…)` inside an `onClick` arrow | none | a write in a function nested in a JSX attribute's function is not classified (a conservative miss) |

The open `callbacks` enumeration keeps its `SC9005` at each literal. The
guaranteed slot is a claim per item, so it holds anyway.

The contract summaries are the generator's output for
`fixtures/package-contracts/owned-computation-callbacks`, with the resource
ids shortened. The manifest bytes are `package-leaf-registration-consumer`'s,
so the closure digest is unchanged. The `solid-js` stub is that fixture's,
verbatim from rc.9, and `App.tsx` type-checks against it with `tsc --noEmit`.
