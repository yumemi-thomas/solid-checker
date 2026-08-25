# Bundled-contract callback consumer

This fixture pins the consumer half of the 2026-08-23 bundled-contract audit:
an omitted `callbacks` field in a bundled contract is a **certified negative**
at consumer call sites, not an absence of information.

`pkg/contracts/bundled/solid-v2/solid-js.json` used to state no callback row for
`flatten`, and the loader deserializes a missing `callbacks` to `Known(vec![])`
rather than `Unknown` (`ContractClaim`'s `Default`), so
`source_discovery.rs`'s `contract_callbacks` map held an empty row set for the
export and `interproc.rs` propagated nothing from the callback to the call site.
The contract now states `callbacks[0]=inline`, proven from
`@solidjs/signals@2.0.0-rc.0` (`dist/dev.js:8654`:
`do { children = children() } while (...)` — synchronous, with tracking
preserved) and probed in all four condition modes by
`scripts/contract-probes.mjs`.

Five cases, covering the core and DOM package claims:

- `Untracked` — the positive. `inline` makes the callback's read the *call
  site's*, and the call site is a component body outside any tracked scope, so
  the `flatten(...)` call itself carries a `strict-read-untracked` violation.
  With the certified-negative contract only the read written inside the arrow
  was reported and the call was clean, which is the exact finding the empty set
  suppressed.
- `Tracked` — the negative. The same export called from compiler-tracked JSX
  puts that same read in a tracked position and reports nothing for the call.
  This is what separates an `inline` attribution from a `deferred` one; a
  `deferred` row would keep the call clean in both cases.
- `WebUntracked` / `WebTracked` — the same positive/negative pair for
  `@solidjs/web`'s `applyRef`. This export is not in the dialect primitive
  table, so its newly explicit `callbacks[0]=inline` bundled row is the only
  source that can attribute the callback read to its call site.
- `Watcher` — the control. `createEffect` is in the Solid 2.0 dialect's
  primitive table, so `native_vocabulary_outranks_contract`
  (`rust/crates/solid-reactive-ir/src/contracts.rs`) never creates a contract
  binding for it and no bundled row can change what it reports.

`.solid-checker/runtime.json` selects `browser`/`import`. The bundled `solid-js`
root entrypoint resolves through host-target conditions no single environment
satisfies at once, so a consumer that selects none of them fails closed with
`SC9005` before any callback row is consulted — which is a fact about that
entrypoint, not about this change.

`solid-js.d.ts` reproduces the published signatures of `flatten`, `createEffect`,
`createMemo` and `createSignal` exactly. `solidjs-web.d.ts` reproduces the
browser `applyRef` signature from `@solidjs/web@2.0.0-rc.0`. `tsc --noEmit` on
this project is clean: a declaration cannot say when a callback runs, which is
the whole reason the contract carries the row.
