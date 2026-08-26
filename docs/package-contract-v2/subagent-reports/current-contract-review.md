# Sub-agent report: current package-contract architecture

**Agent:** `current_contract_review`

**Status:** Read-only review

**Date:** 2026-08-27

**Recommendation:** Redesign

## Scope and authority

The review covered repository policy and terminology, the public schema, Rust
and JavaScript decoding/normalization, the Rust contract model and consumers,
static generation, runtime probing, mechanical verification, bundles, review
artifacts, refusals, structural complexity, and representative serialized size.

Published `solid-js@2.0.0-rc.3` and related `@solidjs/*` artifacts are the
Solid 2 authority. Checked-in RC.0 bundles were used only as structural and size
examples.

## Findings

### Critical: JavaScript normalization can erase conditional behavior

Rust describes `contract_document.rs` as the compact-document owner, but
`packages/cli/scripts/contract-document.mjs` separately expands and normalizes
contracts. Its semantic equality removes nested `variants` and evidence before
comparison, then may collapse variants using only unioned condition tokens.

Two branches with matching base fields but different nested conditional
behavior can therefore compare equal. Token sets such as `{a,b}` and `{b,c}` do
not prove coverage of real resolver conjunctions or fallbacks.

**Risk:** Conditional behavior can be published as unconditional.

**Correction:** Make wire normalization structural and lossless. Perform any
semantic equivalence reduction only in the one resolver-aware normalized model.

### Critical: Omission is currently complete negative without closure proof

Legacy `ContractClaim<T>` defaults missing fields to known default values. For
list domains, omission therefore means an accepted empty list. The generator
also treats omitted callbacks as a negative claim.

Generic discovery probes sample a small callback surface and cannot prove that
no omitted callback exists. A future `closed` field has the same trust problem
if the generator is allowed to write it without independent proof.

**Risk:** A generator omission can silently suppress a real checker finding.

**Correction:** Only a verifier-issued, domain-specific proof may close a
domain. Sidecar hashes alone prove integrity, not exhaustiveness.

### Critical: Artifact identity does not cover inferred behavior

The current schema has at most one package-global declaration artifact and one
implementation artifact. Generation frequently emits neither complete
multi-artifact identity nor declarations and hashes an entry file without the
runtime module closure used for inference.

A barrel can remain byte-identical while an internal implementation module
changes.

**Risk:** Artifact validation can succeed for different behavior bytes.

**Correction:** Bind every resolved case to package integrity, manifest,
runtime artifact, declaration artifact, and canonical dependency/import closure.

### Critical: Environment matching is not package-export resolution

Entrypoint conditions are overloaded as both branch alternatives and asserted
scope. Rust, JavaScript probing, and generation use different matching and
precedence rules. CJS/`require` is rejected, while worker, Deno, Bun, and custom
conditions are incomplete or absent.

**Risk:** Probe and analyzer can select different artifacts.

**Correction:** Record exact resolved cases and select using the actual resolver
result. Host/mode/loader labels must not emulate ordered package exports.

### High: The internal contract model is schema-shaped

`PackageContract` retains string-valued wire concepts and recursive variants.
Consumers directly inspect callback execution, owner strings, sentinel state,
and wire-shaped returns. The native dialect outranks contracts specifically
because the contract cannot represent required ownership, async, write, and
cleanup semantics.

**Risk:** Every consumer must independently interpret fail-closed semantics.

**Correction:** Decode once into typed artifact cases, operation graphs,
recursive values, ownership relations, and local knowledge.

### High: Uncertainty is too coarse

One contradictory callback converts the entire callback domain to unknown. One
unconfirmed return leaf converts the complete return tree. `asyncBehavior` has
no evidence granularity.

**Risk:** Independent sibling proof is discarded, increasing refusals.

**Correction:** Localize knowledge at callback selector, operation, resource,
and recursive value leaf.

### High: Current execution vocabulary cannot describe Solid 2

`inline | tracked | deferred` and one owner label cannot express initial versus
repeated compute, queued apply, errors, async emissions, writes, invalidations,
cleanup production/replacement, or causal ordering.

**Correction:** Use a restricted operation graph with explicit triggers,
scheduling, tracking, ownership, cardinality, resources, and causal edges.

### High: Conditional base summaries can contain false universal facts

Generation merges conditional branches into an environment-independent base,
then adds variants. Some domains are unioned, making a base positive fact false
in at least one branch unless every consumer resolves variants first.

**Correction:** Eliminate semantic conditional bases. Exact artifact cases use
complete summaries; common behavior is deduplicated without semantic merging.

### High: Mandatory binary `kind` is a refusal amplifier

Every export must be `function` or `value`; unknown and mixed callability are
unrepresentable. Unobserved kind can remove an entrypoint or whole document.

**Correction:** Make callability a locally knowable value-shape property.

### High: Inline evidence is large but non-authoritative

Claim evidence is repeated in the main document, while important integrity and
verification data live in a sidecar that ordinary analysis does not use.

**Correction:** Move details to bidirectionally hash-bound sidecars and use a
small verifier-issued receipt during analysis.

### High: Bundled Solid 2 contracts are stale

Checked-in bundles identify RC.0. The current authority is RC.3, and `solid-js`
RC.3 still contains open incompleteness findings. Bundles are compiled through
`include_bytes!`, so edits require a fresh native build.

**Correction:** Regenerate from exact RC.3 artifacts. Preserve unresolved RC.3
domains as open; do not translate RC.0 semantics mechanically.

### Moderate: Documentation and implementation have drifted

RFC text, source comments, current probe implementation, and package-contract
documentation no longer describe one consistent system.

**Correction:** Freeze a normative semantic/normalization specification and
turn load-bearing prose into executable invariants.

## Current architecture summary

- Root legacy document requires schema version 1, compiler protocol 1, package,
  summaries, entrypoints, and evidence.
- Entrypoints group exports under summary IDs or alias another entrypoint.
- Claims are complete known values or whole-domain unknown sentinels.
- Rust expands groups and aliases, rejects unused summaries, and constructs a
  schema-shaped `PackageContract`.
- JavaScript independently expands, mutates, merges, and collapses summaries.
- Generation analyzes supported ESM leaves, walks relative runtime modules,
  merges conditional branches, writes inferred contracts, and creates review
  and probe plans.
- Probing confirms a limited set of already-stated claims in four modes.
- Mechanical verification converts unconfirmed whole domains, refuses
  unobserved kind surfaces, and writes a non-authoritative report.

## Current refusal surface

Known blockers include:

- multi-artifact and closure identity;
- CJS and `require`;
- mixed callability;
- per-callback and recursive-leaf uncertainty;
- operation phases and causal ordering;
- callback argument shapes and owners;
- store paths and most async behavior;
- exact environment selection;
- incomplete JSX typing during generation.

Latest recorded authority:

| Measure | Count |
| --- | ---: |
| Verified | 309 |
| Refused | 90 |
| Generation failures | 18 |
| Driven claims passing | 7,122 |
| Undriven claims | 3,692 |
| Incompleteness findings | 503 |
| Official Solid | 14 / 21 |
| Solid Primitives | 236 / 291 |

## Quantitative baseline

Current schema:

| Metric | Value |
| --- | ---: |
| Pretty bytes | 10,754 |
| Minified bytes | 6,197 |
| Definitions | 6 |
| Named properties | 69 |
| Required-name occurrences | 47 |
| References | 23 |
| `oneOf` / `anyOf` / `allOf` | 9 / 4 / 1 |
| Maximum depth | 13 |

Representative minified normalized contracts:

| Contract | Bytes |
| --- | ---: |
| Solid 1 debounce | 692 |
| Solid 1 rootless | 1,917 |
| Solid 1 scheduled | 2,265 |
| Solid 1 `solid-js` | 6,717 |
| Solid 2 `solid-js` RC.0 | 17,368 |
| Solid 2 signals RC.0 | 415 |
| Solid 2 web RC.0 | 15,888 |

These are not proof-equivalent comparisons with the proposed richer format.

## Migration hazards

1. A current probe-construction sidecar already uses `schemaVersion: 2`; every
   document family needs a discriminator.
2. Final new version 1 collides with legacy version 1; the legacy decoder and
   artifacts must disappear atomically.
3. Old omission means negative while new missing-open means unknown.
4. Raw hash changes invalidate review, probe, verification, and cache state.
5. Bundled normalized and flat review formats both need migration.
6. Compiled bundles require a fresh native build.
7. RC.0 content cannot seed RC.3 semantics.
8. Counter-based summary IDs create needless hash churn.
9. Gate and normalized-model cache versions must move.
10. Contract, probe, receipt, Type Facts, compiler-facts, and cache versions are
    separate namespaces.
11. Repository schema-version policy had to be updated before implementation.
12. Mixed readers and writers are unsafe.

## Recommendations

- Redesign rather than extend the legacy model.
- Make normalization lossless and single-owner.
- Select exact resolved cases.
- Use local four-state set knowledge.
- Require closure receipts.
- Adopt causal operation semantics.
- Keep recursive uncertainty local.
- Remove conditional universal bases and summary overrides.
- Bind evidence bidirectionally and consume a receipt at analysis time.
- Regenerate official contracts from exact RC.3 authority.
- Give every document family a discriminator.
- Treat the final `2 -> 1` transition as an atomic format replacement.
