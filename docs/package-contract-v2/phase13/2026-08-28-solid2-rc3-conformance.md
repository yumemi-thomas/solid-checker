# Phase 13 completion: Solid 2 RC.3 conformance

Date: 2026-08-28

## Outcome

Phase 13 is complete at the private normalized-model boundary. All sixteen
rows in the Solid 2 conformance matrix have exact published authority,
normalized semantics, proof and probe expectations, and positive, clean
negative, partial, refusal, consumer, and TypeScript-oracle fixtures.

This is not the Phase 14 product migration. The public schema, generators,
probe worker/driver, verifier tools, discovery, CLI/WASM surfaces, bundled
contracts, process fixtures, runtime locks, and legacy decoder are unchanged.

## Normalized models

`solid-reactive-ir::contract_semantics::solid2_rc3` encodes:

- split `createEffect` compute/apply/error-handler/cleanup/disposal behavior;
- tracked leaf ownership and owned/unowned `onSettled` cleanup distinctions;
- staged writes, invalidation, synchronous flush callbacks, and drain ordering;
- finite guarded `For`, `Repeat`, `Show`, and `Match` callback shapes;
- plain, Promise, and AsyncIterable computations with emission and cancellation;
- Loading/pending/latest/refresh/affects target relations;
- action transitions, optimistic writes, settlement, and reversion;
- store/setter tuple capability separation, projections, snapshots, and reconcile;
- ref factory/application ownership and cleanup;
- root-local event delegation and disposal;
- exact browser, hydration, SSR string, and single-claim stream cases;
- request/response state and cleanup-bound status/header declarations;
- distinct client-transport and server-in-process server references;
- locally experimental frames cases with all unstable protocol leaves open;
- finite `clientOnly` eager/lazy guards with monotone unresolved selection,
  while the server artifact retains only fallback rendering; and
- exact mixed-framework entrypoint selection with RC.3 incompatibility refusal.

Operations retain independent trigger, event, schedule, tracking, owner source,
owner requirements/capabilities/production, lifetime, and cardinality axes.
Resources and causal edges are explicit. Unknown recursive values and open call
domains remain at their exact leaf. The conformance tests also pin deterministic
semantic digests and prove that one runtime artifact hash change changes the
digest.

## Published authority and closure

The three Solid packages are the audited published `2.0.0-rc.3` artifacts at
git head `af6fee86e6dcfbf41869da2c607c82b1fd0939ce`. The corpus records exact SRI,
manifest, selected runtime, selected declarations, entrypoint conditions, and
file hashes.

Four finite closure identities cover direct signals, Solid root, web/server
subpaths, and the external mixed-framework case. They enumerate exact package
instances and complete file-manifest digests for `solid-js`,
`@solidjs/signals`, `@solidjs/web`, `seroval@1.5.6`,
`seroval-plugins@1.5.6`, `csstype@3.2.3`, and where applicable
`@formkit/auto-animate@0.10.0`. The replay command recomputes every file
manifest before accepting the closure digest.

The auto-animate package is selected only through `./solid`; React, Vue, and
other sibling artifacts are not classified by package keywords. Its exact
Solid runtime imports `onMount` and `onCleanup`, which published RC.3 does not
export. Exact provenance is therefore known while RC.3 compatibility and call
behavior remain open/refused.

## Proof, probes, consumers, and TypeScript

The machine report lists the relevant proof families for every row and pins the
complete eighteen-family closure policy. Each row has a semantic probe recipe
where observable, with `absenceIsNegativeProof: false`. Exact Node/browser-
conditioned replay establishes:

- initial effect compute before the first queued apply;
- replacement cleanup before the next apply and cleanup on disposal;
- tracked and on-settled cleanup ordering; and
- staged signal values remaining settled-old until `flush` drains.

Artifact replay verifies 33 declaration/runtime selectors and hashes. The
sixteen TypeScript oracle cases run independently against the same extracted
published packages. Fifteen exit cleanly. The server-functions client case
exits with the pinned published-declaration error: `ServerFunctionMetadata` and
`ServerFunction` are re-exported but not imported into local declaration scope.
That is TypeScript-owned, produces no checker finding, and opens only that
declaration leaf.

Consumer fixtures query normalized operations, owner relations, resource
states, shapes, guards, artifact cases, and stability. Dynamic guard selection
joins every possible branch and never creates guaranteed behavior. Negative
fixtures assert clean absence of an unrelated semantic claim; refusal fixtures
exercise dynamic helpers, escaped callbacks, wrong artifacts, unaccepted
dependencies, post-commit response mutation, unstable protocols, and the
incompatible mixed-framework adapter.

## Checks

Focused checks completed:

```text
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib
  202 passed
bun scripts/package-contract-v2-phase13.mjs --check
  16 rows valid
bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs scripts/package-contract-v2-phase13.test.mjs
  3 passed
bun scripts/package-contract-v2-phase13.mjs --replay <exact-node_modules> --node <node-24.11.1> --tsc packages/cli/node_modules/typescript/bin/tsc
  16 rows; 33 artifact observations; 16 TypeScript oracles; 3 runtime traces passed
```

Final repository verification:

```text
make verify
  passed in 105.14s
  workspace tests, both backend/WASM feature checks, 94-project coverage
  (557 findings), 289 ownership cases, 161-case TypeScript oracle gate,
  contract conformance, CLI tests, and performance certification passed
```

## Producer and generated-artifact impact

Type Facts producer/protocol, Rust Type Facts client, Solid compiler facts,
compiler pin, and compiler fork did not change. No public schema, compact
contract, receipt, bundled contract, snapshot, dialect manifest, runtime lock,
or generated product artifact changed. The new checked Phase 13 conformance
JSON is evidence input, not a generated public contract.

## Exact remaining open or uncertifiable cases

- Dynamic effect result/error payloads and escaped callbacks remain local.
- Unowned `onSettled` has no certified cleanup lifetime.
- Dynamic keyed/control values join all finite alternatives.
- Async rejection payloads, opaque refresh/action/store targets, and projection
  rejection details remain open where exact facts do not select them.
- DOM ref timing, delegated-root lifetime, hydration asset settlement, and
  `clientOnly` mount timing require a replayable real-browser authority before
  those observation leaves can close.
- Request-context status/header and end-to-end server transport observations
  remain open without their bounded integration harnesses.
- Server-function user serialization and server synchronous-result leaves are
  open; the published client declaration defect is TypeScript-owned.
- All unstable frames protocol call domains remain open and experimental.
- The exact auto-animate Solid adapter is RC.3-incompatible and therefore
  refuses behavior certification despite exact framework provenance.
- Phase 14 still owns the atomic public producer/consumer migration and bundle
  regeneration. Ordinary analysis does not consume this corpus yet.
