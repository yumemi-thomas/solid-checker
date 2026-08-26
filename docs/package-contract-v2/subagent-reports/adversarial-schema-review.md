# Sub-agent report: adversarial review of the proposed schema

**Agent:** `schema_attack`

**Status:** Read-only review

**Date:** 2026-08-27

**Verdict:** Redesign

## Scope and authority

The review covered repository policy, the current schema and normalizers,
package-contract documentation and RFCs, generator/probe/verifier machinery,
Solid 2 skill references, and published RC.3 package exports, declarations, and
runtime implementations.

Inspected package authorities included:

- `solid-js@2.0.0-rc.3`;
- `@solidjs/signals@2.0.0-rc.3`;
- `@solidjs/web@2.0.0-rc.3`;
- `@solid-primitives/trigger@3.0.0-next.2` as a dependency-closure
  counterexample.

Hashes in the appendix are verified published-file identities. Any digest
marked `PLACEHOLDER` in an illustrative schema fragment is not an attestation.

## Findings

### Critical: Proposed environment axes are not export resolution

Published RC.3 packages use ordered nested branches including worker, browser,
Deno, Node, development, import, require, types, and default. A conjunction of
host/mode/loader can match multiple schema cases where the real resolver selects
one earlier branch.

**Correction:** Resolve first, then match exact runtime/types branch trace,
resolved targets, hashes, and closure. Cases never merge.

### Critical: `at` and flat phases cannot express Solid 2 execution

`createEffect` computes initially in the creating stack, queues apply, recomputes
on invalidation, can land Promise/AsyncIterable results repeatedly, executes
success/error arms, and replaces/disposes returned cleanup. `flush(fn)` also
separates callback execution from draining queued writes.

**Correction:** Use a restricted operation graph with trigger, schedule,
cardinality, tracking, ownership, guards, and causal edges. Phase syntax may be
authoring sugar only.

### Critical: Call-level closure can hide omitted operations

If callbacks remain nested in phases while `closed` lives at call level, a
missed phase can become false absence. Naming `reads` in `closed` without an
explicit sibling collection has the same risk.

**Correction:** Every set domain is an immediate sibling of its governing
`closed`; a closed empty domain is explicit `[]`; open empty arrays are invalid.

### Critical: Positional callback identity is insufficient

Solid callbacks come from positional arguments, object members such as
`effect`/`error`, component children, returned factories, and result paths.

**Correction:** Add exact source selectors such as
`{ "arg": 1, "path": ["effect"] }` and result bindings/paths.

### Critical: A sidecar hash is integrity, not authority

A compromised generator can write a false contract and matching false sidecar.
Stale contract and sidecar bytes can also agree while describing an old
artifact.

**Correction:** Bind exact semantic subject, artifact closure, stable claim IDs,
tool identity, and proof root. Require independent verification or explicit
trusted authority before accepting closure.

### High: Ownership dimensions are conflated

Inherited, created, leaf, detached, and none answer different questions about
source, parentage, capabilities, and lifetime. They cannot describe effect
compute/apply, `onSettled`, two-phase refs, roots, or request scope precisely.

**Correction:** Separate owner source, parent relation, child policy, cleanup
capability, lifetime, production, and requirements.

### High: Conditional behavior needs restricted guards

Keyed control flow, effect bundles, owner-dependent `onSettled`, async outcomes,
server/client server functions, development diagnostics, and response
commitment branch behavior without necessarily changing an artifact summary.

**Correction:** Guard operations and recursive choices with exact finite facts.
Unknown guards join all possible branches; negatives require every branch.

### High: Recursive shape knowledge needs local closure

Tuple, object, Promise, AsyncIterable, and conditional result shapes can have
known siblings beside unknown leaves.

**Correction:** Use tagged recursive shapes with object/choice-local knowledge.
Unknown leaves do not contaminate siblings.

### High: Nominal brands and behavior are mixed

Snapshots are plain values; deep reads are operations; projections are
store-like plus refresh semantics; optimistic state adds transition effects.

**Correction:** Keep accessor/setter/store as the small reactive role set.
Represent draft, reconciliation, deep tracking, refresh, optimistic commit, and
revert as operations. Keep observable protocol brands separate.

### High: Own artifact hash is insufficient

`@solid-primitives/trigger` can load the same own artifact while behavior changes
through the resolved `@solidjs/web` dependency. Mixed-framework packages expose
the same problem.

**Correction:** Bind the exact reachable dependency/import closure per case and
derive framework applicability from provenance, never a manual label.

### High: Pending, transaction, response, and transport effects need semantics

`affects`, async landing/yield, actions, optimistic commit/revert, response
commitment, and server transport are not adequately described as ordinary
reads/writes/invalidations.

**Correction:** Allow operation effects for pending state, transaction
lifecycle, response mutation, transport, async outcomes, and cleanup binding.

### High: Summary overrides make behavior non-local

Deep overrides create merge-order semantics, closure ambiguity, evidence
identity drift, and decoder disagreement.

**Correction:** One exact reference or one inline summary; no override or deep
merge.

### Medium: Experimental status must be local

Published server components are experimental.

**Correction:** Attach `stability: "experimental"` to exact export/case identity
or omit/refuse that surface. Absence is unknown, not stable.

### Medium: Compactness is unsafe unless normalization is genuinely deep

Adding closure, guards, shapes, cases, and proof bindings while consumers still
inspect wire fields merely spreads schema complexity.

**Correction:** One normalization entry point consumes raw contract, actual
resolver facts, local artifact facts, and acceptance facts and returns only
normalized semantics or scoped refusals.

## Corrected minimal direction

Keep:

- required format discriminator;
- temporary schema version 2;
- exact package identity;
- exact resolver-derived artifact cases;
- shared summaries without overrides;
- immediate local closure;
- operation graph;
- exact callback selectors;
- recursive shapes and choices;
- hash-bound sidecars and acceptance receipt.

Delete:

- `schemaStatus`;
- `compilerFactsProtocol` from the main contract;
- authoritative host/mode/loader matchers;
- vague `deferred` timing;
- cleanup as a time;
- nominal projection/snapshot reactive kinds;
- redundant generic readable capability;
- summary overrides;
- manual framework labels;
- per-fact `{status,value}` wrappers.

## Knowledge lattice

| State | Encoding |
| --- | --- |
| Unknown | Domain absent and not named in local `closed` |
| Partial positive | Non-empty domain present and not closed |
| Complete positive | Non-empty domain present and closed |
| Complete negative | Explicit empty domain present and closed |
| Invalid | Empty open domain, missing closed sibling, duplicate item/name |

Closure never inherits into child objects, choices, guards, references, or
sibling domains.

## Normalization invariants

- Exact format and version are required; `schemaStatus` is forbidden.
- Package integrity, manifest, artifact, declarations, and closure bind every
  usable case.
- Actual resolver result selects exactly one case.
- Identity mismatch refuses a case.
- Semantic incompleteness opens a local domain.
- Every closed name has an explicit immediate sibling collection.
- Operation IDs are unique and ordering edges acyclic.
- Every semantic fact references a valid operation/resource.
- Complete guards are disjoint and exhaustive.
- Recursive knowledge remains local.
- Capability contradictions are rejected during normalization.
- Summary references are acyclic and have no overrides.
- Consumers never inspect raw closure, schema names, resolver syntax, or proof
  envelopes.

## Closure proof obligations

Before closing a claim domain, establish:

1. exact package/version/integrity and manifest;
2. exact runtime and declaration resolution branches and targets;
3. complete reachable runtime/dependency closure;
4. exact export and declaration identity;
5. complete relevant call/reference/return enumeration;
6. no unresolved dynamic dispatch or callback escape;
7. exhaustive/disjoint guards;
8. initial, update, repeated, error, async, transition, and disposal paths where
   relevant;
9. recursive shape traversal with leaf locality;
10. callback source and cardinality;
11. cleanup capture, replacement, disposal, and owner binding;
12. request/DOM/owner/tracking/response preconditions;
13. mixed-framework reachability;
14. no probe contradiction;
15. stable claim identity bound to exact tools and artifacts.

Probes may confirm positives or falsify closure, but never prove absence.

## Migration hazards

- Existing agent policy originally required schema version 1; it must reflect
  the approved temporary-version migration.
- New stable version 1 collides with legacy version 1; a required format
  discriminator and atomic decoder replacement are mandatory.
- Renumbering changes main bytes, sidecar bindings, signatures, caches, and
  compiled bundles.
- Producer/consumer, contract/sidecar, and cache versions can mismatch during a
  partial rollout.
- Final stable bytes need fresh hashes and receipts even if semantic evidence is
  reusable.
- Rollback must restore the complete producer/consumer/artifact set.

## Compactness assessment

Legacy canonical minified examples are approximately:

| Contract | Bytes |
| --- | ---: |
| Solid Primitives rootless | 1,918 |
| Solid 2 `solid-js` RC.0 | 17,369 |
| Solid 2 signals RC.0 | 416 |
| Solid 2 web RC.0 | 15,889 |

These documents are not proof-equivalent. The new format pays for exact
resolver leaves, operations, recursive shapes, and identity, while removing
inline evidence, override trees, and Cartesian environment cases. Measure
canonical p50/p95, bytes per normalized claim, and sidecars separately after
migration.

## Verified published artifact identities

### `solid-js@2.0.0-rc.3`

- integrity: `sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==`
- `package.json`: `e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b`
- `dist/dev.js`: `dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18`
- `dist/solid.js`: `14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b`
- `dist/server.js`: `63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8`
- `types/index.d.ts`: `76b94bfb3a95099405a8cae461fff7b83c5a3cd61667cf72c23e7f850cf52740`

### `@solidjs/signals@2.0.0-rc.3`

- integrity: `sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`
- `package.json`: `22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76`
- `dist/dev.js`: `cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79`
- `dist/prod/index.js`: `5c0a6384d330cfdf979197f0c6037bbb1db9712e3fe5d4cafb2de886dd509907`
- `dist/node.cjs`: `bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c`
- `dist/types/index.d.ts`: `e4157c4caba48476db4e7649b5a50827687c2d90c0a09fa091f0e65d0a63cfb4`

### `@solidjs/web@2.0.0-rc.3`

- integrity: `sha512-5ckKgOjem1pN5ADycOk6TjHmTtjbbN2fukqxo6RW3Oe3H7z0gaXWAdt8dLISto5/O4Nn8VxprFXFWpfy31+DUg==`
- `package.json`: `ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e`
- `dist/dev.js`: `d848d00341ac8195e191404ace7dd8b4c650f47befb0cfecac78ddcf01587851`
- `dist/web.js`: `3eccc22880306613c83a658d5889f9b307fad4a114c8842e12b9db5ffe46bf27`
- `dist/server.js`: `80abb46a98a9d6695b7d2c42725ccfb538f8e941d6aa3a8ec5343d6d002d54b1`
- `types/index.d.ts`: `5870c51be7674969670ccb084077d3df29ed732db8e8ad03527d384285c99635`

### `@solid-primitives/trigger@3.0.0-next.2`

- integrity: `sha512-YhTGBKEPP7XI7Bk6/2gWxfZmwMWQEwfZsTiMO2dt9X/AQ9bmLk+xuRbT2QDOPg2IBe0D3uh/uguHUMYOLYGpbQ==`
- `package.json`: `857c14b6605d16902777bdd3d53ded0547fd0c253e14dc9bad43966842ae4780`
- `dist/index.js`: `f803d7322d9ea2c1bde9bdfe1769de057089d01da034f160c535cf4f3ec286ac`
- `dist/index.d.ts`: `1d07aa983e841026650ae6b7bf57f5336cecc095bd00abbd26ac5c3b3b28afff`

These hashes establish file identity only; they are not semantic attestations or
closure proofs.
