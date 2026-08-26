# Sub-agent report: Solid 2.0.0-rc.3 evidence

**Agent:** `solid_rc3_evidence`

**Status:** Read-only published-artifact review

**Date:** 2026-08-27

**Repository changes:** None

## Conclusion

The published RC.3 evidence requires four corrections before package contracts
can certify Solid 2 accurately:

1. Flat phases must normalize to guarded operations with explicit triggers,
   cardinality, scheduling, cleanup, transitions, and stream lifetime.
2. Scalar ownership must split call preconditions, execution-owner source,
   created owner, owner capabilities, and cleanup lifetime.
3. Environment selection must follow actual ordered package-export resolution
   and exact artifacts rather than host/mode/loader matching.
4. Certification must bind the resolved transitive module closure, dependency
   instances, declarations, and conditional-resolution paths—not only the
   package's top-level artifact.

The compact local `closed` encoding remains viable only when closure belongs to
one exact claim domain, guarded operation, or recursive shape node.

## Published authority

The workspace Solid skill was audited against RC.0 and was used only to route
topics. Prerelease-sensitive conclusions were rechecked against published RC.3
tarballs, manifests, runtime files, and declarations.

| Package | Registry | Tarball | Published integrity |
| --- | --- | --- | --- |
| `solid-js@2.0.0-rc.3` | [metadata](https://registry.npmjs.org/solid-js/2.0.0-rc.3) | [tarball](https://registry.npmjs.org/solid-js/-/solid-js-2.0.0-rc.3.tgz) | `sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==` |
| `@solidjs/web@2.0.0-rc.3` | [metadata](https://registry.npmjs.org/%40solidjs%2Fweb/2.0.0-rc.3) | [tarball](https://registry.npmjs.org/@solidjs/web/-/web-2.0.0-rc.3.tgz) | `sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==` |
| `@solidjs/signals@2.0.0-rc.3` | [metadata](https://registry.npmjs.org/%40solidjs%2Fsignals/2.0.0-rc.3) | [tarball](https://registry.npmjs.org/@solidjs/signals/-/signals-2.0.0-rc.3.tgz) | `sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==` |

All three packages publish the same usable registry `gitHead`,
`af6fee86e6dcfbf41869da2c607c82b1fd0939ce`. A fresh Phase 0 audit verified
that commit is contained by official `solidjs/solid#next` and that its three
source `package.json` files are byte-identical to the published manifests.
Generated runtime and declaration files are not tracked at that source commit,
so registry tarballs and SRI remain their reproducible authority.

## Verified runtime and declaration hashes

### Runtime and manifests

| File | SHA-256 |
| --- | --- |
| `solid-js/package.json` | `e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b` |
| `solid-js/dist/dev.js` | `dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18` |
| `solid-js/dist/solid.js` | `14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b` |
| `solid-js/dist/server.js` | `63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8` |
| `@solidjs/signals/package.json` | `22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76` |
| `@solidjs/signals/dist/dev.js` | `cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79` |
| `@solidjs/signals/dist/prod/index.js` | `5c0a6384d330cfdf979197f0c6037bbb1db9712e3fe5d4cafb2de886dd509907` |
| `@solidjs/web/package.json` | `ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e` |
| `@solidjs/web/dist/dev.js` | `d848d00341ac8195e191404ace7dd8b4c650f47befb0cfecac78ddcf01587851` |
| `@solidjs/web/dist/web.js` | `3eccc22880306613c83a658d5889f9b307fad4a114c8842e12b9db5ffe46bf27` |
| `@solidjs/web/dist/server.js` | `80abb46a98a9d6695b7d2c42725ccfb538f8e941d6aa3a8ec5343d6d002d54b1` |
| `server-functions/dist/client.js` | `f7e754c2119449c94a01b16760efe1cc9fd4bf53f3f17033392a07b4d6bd00a1` |
| `server-functions/dist/server.js` | `9b5fa3af266d0ae563811a4296f58443ebdf0c79186ee89e416a63e561294201` |
| `server-functions/dist/server.dev.js` | `e1fc68d86022e2d26d9e6a24150001c1e69aaeadedf58398f1480304d948040c` |
| `frames/dist/client.js` | `2e2590acbb2a2fc190e1804c93fb6fbf5f2b686758cdb10c0618399028433a53` |
| `frames/dist/client.dev.js` | `fdd6836e1dee13ac4f04d8c78eae7def8c85d997fdc2bd0b5cdc9419754cfe53` |
| `frames/dist/server.js` | `620ff6ce77756a3151f654b06b0acc9492048ef8ca306289278715117b750717` |

### Declarations

| File | SHA-256 |
| --- | --- |
| `solid-js/types/index.d.ts` | `76b94bfb3a95099405a8cae461fff7b83c5a3cd61667cf72c23e7f850cf52740` |
| `solid-js/types/client/flow.d.ts` | `81af6e73951ea01b3c28b2c2ded7537522b0ebe67575e33f23ba10aed2f1148a` |
| `@solidjs/signals/dist/types/signals.d.ts` | `c3dd3b2a247183379baaddf140fcdc67e3ac563a97b9af3151646a40471de068` |
| `@solidjs/signals/dist/types/core/action.d.ts` | `3643790df75007a043a3ee2cf1bde96eb093ba5ef3edcfe16a827111e3bc1fab` |
| `@solidjs/signals/dist/types/store/index.d.ts` | `4465d814273b75c09f33c4f0d1cbcc18f916373e68898546439bfa3b0dc0b440` |
| `@solidjs/signals/dist/types/store/store.d.ts` | `d581d0958af47625d753d2438db88701e3404ecf97c49333ce26d4b5daa67479` |
| `@solidjs/web/types/client.d.ts` | `49d281ca558aa359a44bed4ce1fb9cc9b54d65cd353b1d3191ab62d350e18c13` |
| `@solidjs/web/types/index.d.ts` | `5870c51be7674969670ccb084077d3df29ed732db8e8ad03527d384285c99635` |
| `@solidjs/web/types/index.server.d.ts` | `54e8367b616bf995f2b85b06a9f0b58e26faf9a4be601741154c51d63a05df25` |
| `@solidjs/web/types/server.d.ts` | `b9a62f469abc1b861720da806d3a2bb410e5de1ff1bf511c6acfbe1b0e4c6a6f` |
| `server-functions/client.d.ts` | `fdce7ef458dc83b823f883e0af58a4cfa75dd77f1889423d1fa03bd771d17eb6` |
| `server-functions/server.d.ts` | `0459be58eb62d4ba980d06864b11df7b59e8d81fe8c490b81b64412f83b37fbf` |
| `frames/client.d.ts` | `e83afca019249516e25679192d747a2de2f0f93684ac38d25b091717516df33b` |
| `frames/server.d.ts` | `c58fa5f4ec79dfc0707d1f01ddc7b464105710ccb9df60fd7db8a3004fb5a5f6` |

Hashes prove file identity only, not semantic closure or truth.

## Conditional exports

Node conditional exports traverse ordered objects; earlier keys have priority,
nested objects continue ordered selection, and custom conditions may be active.
See the official [Node documentation](https://nodejs.org/api/packages.html#conditional-exports).

Published trees differ by subpath:

- `solid-js` and `@solidjs/web` roots broadly order worker, browser, Deno,
  Node, development, import, and require.
- `@solidjs/web/server-functions` nests development inside worker/Deno/Node,
  while browser selects client transport.
- `@solidjs/web/frames` has a different development arrangement.
- `@solidjs/signals` nests test/development under import while require selects
  CJS directly.

Consequences:

1. Active Node+development+import selects the earlier Node server branch for
   the `solid-js` root.
2. The same conditions select the nested server-development artifact for the
   server-functions subpath.
3. They select the ordinary Node server artifact for frames.
4. Custom browser plus Node conditions can be simultaneously active; ordered
   resolution remains unambiguous even though predicate matching finds two
   cases.
5. Generic import fallbacks overlap host-specific import branches under naïve
   matching.
6. The custom signals `test` condition is not representable by host/mode/loader.

The analyzer must resolve first and select by exact runtime artifact and branch
trace. Runtime and declaration resolution are independent; the selected runtime
target does not imply which declaration target TypeScript chose.

## Resolved closure is essential

Top-level runtime files import behavior from separately resolved packages:

- `solid-js/dist/dev.js` imports and reexports `@solidjs/signals` through a
  non-exact dependency range.
- `@solidjs/web/dist/dev.js` imports and reexports `solid-js`.
- `@solidjs/web/dist/server.js` imports `solid-js`, `seroval`, and plugins.
- frames import Solid, web, and server-functions artifacts.

Two installations can therefore share top-level bytes while resolving different
dependency or peer instances. Closure identity must cover package instance,
version/integrity, imported runtime artifacts, conditional paths, finite dynamic
imports, declaration inputs, and any explicit unbounded frontier.

## Required Solid behavior evidence

### Split `createEffect`

RC.3 creates the effect node and immediately performs initial recompute. Apply
is queued unless deferred, later computes are invalidation-driven, success/error
arms are conditional, prior cleanup runs before later apply and disposal, and
async values add later landings/emissions. Apply uses the ambient drain owner;
it is not a fixed leaf owner.

**Implication:** guarded operations must distinguish initial inline compute,
later compute, queued apply, errors, repeated emissions, and cleanup edges.

### `createTrackedEffect` and `onSettled`

Tracked effects are children-forbidden leaf computations with returned cleanup.
Owned `onSettled` creates a tracked leaf and invokes the callback untracked;
unowned `onSettled` queues a one-shot without a usable cleanup lifetime.

**Implication:** callback owner source, created owner, child capability, call
precondition, and cleanup lifetime are separate guarded facts.

### Batching and `flush()`

Writes stage pending values. `flush(fn)` runs `fn` synchronously and drains in
`finally`; behavior differs inside tracked effects, settled callbacks, effect
apply, production, and the server artifact.

**Implication:** distinguish stage, commit, drain, callback invocation, explicit
versus automatic drains, transition drains, errors, warnings, and no-ops.

### `For`, `Repeat`, `Show`, and `Match`

RC.3 declarations and runtime distinguish raw versus accessor item/value and
numeric versus accessor index across keyed modes. List rows have owner/lifetime
semantics, and mapping can retry after async suspension.

**Implication:** use finite guarded callback shapes plus row owner/cardinality,
not one unconditional positional callback.

### Promise and AsyncIterable computations

Compute functions may return plain values, Promises, or AsyncIterables. The
runtime can consume streams repeatedly, cancel them through `iterator.return`,
flatten a Promise to an AsyncIterable, handle errors, and supersede stale work.

**Implication:** distinguish returned async protocols from internally consumed
async sources and encode emission/completion/error/cancellation operations.

### Loading, pending, latest, refresh, and affects

Initial loading values, later revalidation, transitive pending probes, detached
latest shadows, branded refresh targets, store-key affects, and server no-op
behavior are different operations. A plain signal may be accepted by refresh
yet have no computation to invalidate.

**Implication:** pending/refresh are guarded target/resource relations, not a
flat capability bag.

### Actions and optimistic state

Actions drive generator steps in transitions, reenter after yielded settlement,
flush steps, and resolve/reject a returned Promise. Optimistic signal/store
values are lane-owned and commit or revert; the server action wrapper is much
simpler.

**Implication:** model transaction identity, yielded resumption, transition
hold/commit/revert, optimistic target, authoritative landing, and owner
preconditions.

### Stores, projections, snapshots, deep/shallow behavior

Store values are read-only while sibling setters are writable and supply
temporary drafts. Derived stores may be refreshable; projections may be async;
deep reads produce snapshots; optimistic visibility and reconciliation are
operations.

**Implication:** describe tuple/callable relationships and resources. Snapshot
is plain output, projection is behavior/provenance, and writable/refreshable/
optimistic must not be assigned indiscriminately to one store leaf.

### Two-phase refs/directives

The renderer evaluates a ref factory untracked while retaining setup owner,
flattens returned callback arrays, and applies callbacks under no owner.

**Implication:** returned callable shapes need their own invocation semantics,
element argument identity, setup owner, application owner, and cleanup relation.

### Root-owned event delegation

Browser render creates a reactive root and delegated root, then returns a
disposer that disposes, unregisters, and clears the container. Portals can add
delegated containers with their own cleanup.

**Implication:** explicit resource identities and cleanup target relations are
required; delegation is not process-global.

### Browser render/hydrate and server rendering

Browser render/hydrate behavior differs from server render-to-string/stream.
Hydrate may invoke rendering immediately or after module assets settle. Browser
SSR helpers are unsupported, while server stream results enforce mutually
exclusive consumption modes.

**Implication:** artifact cases and guarded operations are both required.
Stateful stream consumption needs limited typestate.

### `httpStatus` and `httpHeader`

Server helpers require a current request/response, apply only before commitment,
record response-ledger entries, and retract declarations through owner cleanup.
Client declarations describe no-op behavior.

**Implication:** request scope, response commitment, ledger target, owner, and
retraction are execution state—not package environment.

### Server-function references

Client artifacts create fetch-backed proxies with encoding, decoding, errors,
streaming cancellation, and reconnecting live AsyncIterables. Server artifacts
proxy original functions in request scope and may preserve synchronous returns.

**Implication:** callable protocol identity is separate from client transport or
server in-process operations, preconditions, async result, and cancellation.

### Experimental server components

Frames declarations mark server-component installation/slot APIs experimental,
require explicit client installation, and depend on matching client/server
artifacts and compiler transformation.

**Implication:** stability comes from declaration/release evidence, never probe
success. Missing integration preconditions cause refusal.

### Conditional adapters

Hydrate timing, server-function client/server behavior, and keyed control flow
all branch without necessarily changing a whole behavioral summary.

**Implication:** reuse shared operations with guard-local alternatives and
closure. Complete one branch never closes another.

### Mixed-framework packages

`@formkit/auto-animate@0.10.0` publishes separate Solid, React, Vue, Preact,
Marko, and Angular artifacts. Package keywords are not provenance, and its Solid
adapter imports the removed `onMount`, so even exact Solid adapter provenance
does not establish RC.3 compatibility.

**Implication:** framework applicability comes from exact entrypoint/artifact
and dependency closure, including compatible Solid version—not labels.

## Cross-cutting implications

| Proposed simplification | RC.3 falsifier | Required representation |
| --- | --- | --- |
| Flat phases | Initial/repeated effect compute, scheduled apply, cleanup triggers, streams | Guarded operation graph |
| `at` enum | Transition settle/revert, emission, cancellation, response commitment | Trigger plus scheduling and causal edges |
| Scalar owner | Ambient drain owner, guarded `onSettled`, row roots | Separate requirements, execution owner, created owner, capabilities, lifetime |
| Capability bag | Store/setter split, lane-bound optimism, refresh target relations | Structured operations/resources and validated capabilities |
| Host/mode/loader | Ordered nested custom conditions | Actual resolver result and exact artifact |
| Top-level artifact hash | Range-resolved dependencies and peers | Resolved transitive closure digest |
| Call-level closure | Keying, owned/unowned, hydrate branches | Local guard/operation/shape closure |
| Evidence hash | Self-consistent compromised generator output | Independent proof checker and receipt |

## Closure obligations derived from RC.3

Before closure, enumerate as applicable:

- exact package instance, integrity, specifier, runtime/declaration resolution,
  artifacts, and dependency closure;
- aliases, reexports, overloads, runtime argument/prop branches, callback
  positions, rest parameters, retained callbacks, and returned factories;
- initial and repeated behavior, scheduling versus actual invocation, success,
  error, cancellation, cleanup-before-next, and disposal;
- Promise/AsyncIterable settlement, emissions, supersession, and iterator
  closure;
- transition commit/revert and optimistic target ownership;
- recursive shape leaves and target/resource aliases;
- real-browser authority where browser facilities matter;
- exact claim/operation/guard evidence selectors;
- independent verification of generator-proposed negatives.

No observation may be converted into proof that unobserved behavior is absent.

## Current gaps and refusals

Recorded RC.3 generation summaries include:

| Package | Exports | Proven | Exports with unknown |
| --- | ---: | ---: | ---: |
| `solid-js` | 82 | 30 | 52 |
| `@solidjs/signals` | 61 | 35 | 26 |
| `@solidjs/web` | 515 | 451 | 64 |

The Solid 2 verification report records 191 verified, 54 refused, four
generation failures, and one no-runtime row across 250 rows. Unobserved legacy
binary `kind` is a major schema-form blocker, while evidence-write failures are
workflow blockers rather than missing semantic vocabulary.

Known schema-limited behavior includes variadic `merge`/`mergeProps`, returned
conditional adapters, narrow callback-result relations, unsupported named/rest
callback argument descriptors, parameter-attributed writes, and whole-return
unknown contamination. Local recursive knowledge, guarded operations, callable
return behavior, parameter/resource relations, and universal rest selectors are
the appropriate remedies.

`@solidjs/vite-plugin` remains a genuine closure refusal where an unbounded
dynamic import prevents complete module closure. A closure digest can preserve
that fact but cannot prove beyond the frontier.

## Remaining evidence questions

- Which exact resolver implementations are authoritative for Node, bundlers,
  browsers, Deno, Bun, workers, and edge deployments?
- When identical bytes execute under different host facilities, must resolver
  trace remain part of case identity?
- How is an unbounded dynamic-import frontier scoped to affected exports?
- What is the explicit runtime/declaration conflict policy?
- Which metadata sources may assert experimental stability?
- Which browser claims require a real browser rather than the inert shim?
- How are universal callback/rest positions encoded compactly?
- Which independent verifier or authority accepts negative closure?
- How do claim IDs survive harmless summary/compression changes?
- How are v2 evidence and receipts prevented from replay after stable-v1
  renumbering?
