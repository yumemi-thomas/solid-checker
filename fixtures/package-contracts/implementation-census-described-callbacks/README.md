# implementation-census-described-callbacks

The tracer for ADR 0152
(`docs/adr/0152-a-described-callable-may-invoke-its-exports-argument.md`): an
export that hands back a fresh function literal calling an argument the export
was handed proposes two claims, and the certifier proves or withdraws each.

- **`returns`**: the literal's described callable gains a nested `callbacks`
  item per captured argument it calls -- that argument runs exactly once per
  invocation, on the invoker's stack, in the invoker's tracking context and
  under its owner -- and a completion that is exactly such a call hands back
  `invocation-result` of it.
- **`callbacks`**: the export's own claim keeps each such argument at ADR
  0139's `result-access` event: the export's call runs none of them and keeps
  them only in the value it returns.

The generated side is pinned by this fixture's `expected.json`. The
certifier's side is pinned by
`the_described_callback_census_certifies_exactly_the_unconditional_captured_calls`
in `rust/crates/solid-facts-backend/src/contract_certification.rs`, which plans
the same package with each export's described callables as the walk proposes
them and a `result-access` item per captured argument (for `defaulted` too,
which the walk does not propose), and certifies against the real producer with
the synthesized vetoes as the only probes:

| export | `returns` | `callbacks` |
| --- | --- | --- |
| `pipe` (`@solid-primitives/utils`, byte for byte) | certifies: items for 0 and 1, `invocation-result` of 1 | certifies: `result-access` for 0 and 1 |
| `changed` (`@solid-primitives/promise`, byte for byte) | certifies: item for 0, `plain` | certifies: `result-access` for 0 |
| `required` | certifies: a `throw` before the call leaves every normal completion running it | certifies |
| `registered` | certifies | refused: the export also hands the argument to `registry.push` |
| `guarded` (`callback && callback()`) | withdrawn: the call is not unconditional | refused: the guard's read is no call |
| `early` (`if (flag) return; callback()`) | withdrawn: the call is not unconditional | certifies |
| `twice` | withdrawn: called more than once per invocation | certifies |
| `deferred` (`queueMicrotask(() => callback())`) | withdrawn: the call sits in a callable the literal nests | refused: not in the returned literal's own frame |
| `defaulted` (`callback = fallback`) | withdrawn: a defaulted parameter is no binding identity | refused: no call of the parameter by binding identity |

`index.d.ts` declares `pipe` exactly as `@solid-primitives/utils@7.0.0-next.4`
does and `changed` as `@solid-primitives/promise@2.0.0-next.2` does, with
`Accessor<T>` written out as the function type solid-js declares it to be.
