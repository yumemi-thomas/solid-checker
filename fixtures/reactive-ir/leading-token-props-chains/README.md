# leading-token-props-chains

**Claim (ADR 0376).** A props or store read is classified from exact spans only, never from the leading token of a compound expression:

- `props.a.b` and `props.get().b` read the prop `a` / `get` at the chain's head (certified when static), not the prop `b` (before: violations through prop `b`). Only a store reads through a chain of static members (`store.items.length`, its nested proxies).
- A receiver that may evaluate to `props` (`(view && props).b`, `(flag ? plainPair : props).b`) is an obligation; it was dropped (certified).
- A caller's `(store ? first : second).name` only tests the proxy and `count.toString()` calls no accessor: unknown callers, uncertifiable (before: reactive, violations). `fmt={count.toString}` is not the accessor (before: SC8015).
- An effect apply's parameter is the store only when the compute returns the store or a static member of it (`() => store.items`, a violation at `value.length`); `() => (store.open ? "yes" : "no")` returns a string (before: a store-path violation).
- `merge(store.open ? small : large, …)` views plain objects (before: a store read and a "store spread").
- `pick(el && entries, "a")` may hand `pick` the store: SC9012 (before: certified through `el`).

Controls: `props.b`, `String(count())`, `() => store.items`, `pick(entries, "a")` and the `store.open` tests keep their findings. Owners: `discover_member`, `argument_proves_non_reactive` in `local_access.rs`; `reference_symbol`, `view_root`, `member_chain_identifier_root` in `source_discovery/typed_sources.rs`; `discover_effect_store_parameters`; `classify_passed_expression`, `passed_expression_is_accessor` in `props_reactivity.rs`. Stub: `argument-sensitive-reads`' plus rc.13 effect declarations from `deferred-effect-apply`'s.
