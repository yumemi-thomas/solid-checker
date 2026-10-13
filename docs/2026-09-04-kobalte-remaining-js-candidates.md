# Kobalte alpha: the remaining four JS candidates reach census refusals

Date: 2026-09-04

The next measurement after ADR 0010 addresses `getScrollParent` and
`isPointInPolygon` in both published JS artifact cases of
`@kobalte/utils@2.0.0-alpha.0`. **All four refuse before the runtime veto.**
Adding recipes alone cannot certify these closures. A separate control run
still certifies `clamp` in both cases. No checker or harness behavior changes
in this follow-up.

## Method and exact inputs

Use the retained alpha installation and authenticated registry cache from the
earlier investigation. The archive integrity is
`sha512-LtMNLEDDJICPJXZiy5yQL23cBfbYSqzT02ItXky9sAaiLfVlasvguE5MmC/9BxC0FnwKrTDU1R3ysM18+TGd0A==`.
The published `dist/index.js` SHA-256 is
`13414950b5a42bc1266d7b11668b5c8e9b2a5a1f8390db57d0dee83680d5245e`.
The native snapshot root remains
`sha256:a7e8e1e2ac7667d6cec41d796bef08798f28fb868da03a35be47d4bee33014e1`.

Call the real `certifyContract` with `--entrypoint .`, the exact package root
and integrity, the existing scratch issuer, and separate catalog/trust/audit
outputs. Each run has a corpus addressing **one** of the four claim IDs below.
The other closures remain open in that diagnostic run. Both root artifact
cases are generated; each audit is checked to contain exactly one
`domain-exhaustiveness` demand, matching the demand named by its refusal.
Thus an earlier case's census cannot mask another candidate's outcome.

The scratch scripts are `/private/tmp/kobalte-remaining-js.mjs` and
`/private/tmp/kobalte-remaining-control.mjs`. After `make build-checker-debug`,
run each with this environment, from the repository:

```sh
SOLID_CHECKER_REGISTRY_CACHE=$PWD/rust/target/registry-cache \
SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/debug/solid-checker-rust \
SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
SOLID_CHECKER_PROBE_NODE=/Users/thomas/.local/share/mise/installs/node/24.11.1/bin/node \
bun /private/tmp/kobalte-remaining-js.mjs
```

The injected fetch callback throws on any cache miss. All four measurements
and the control assert **zero cache misses**; no installation or network fetch
was performed. An initial invocation omitted the cache environment and stopped
at the offline callback before semantic measurement; it is not a census result.

Outputs live under
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/remaining-js/`.
Its `results.json` SHA-256 is
`90e4be63e8d850d775f6afbeea1dc74f0e9d6ae9869544fd56ea2271dc8441aa`.
Each result records its exact refusal, audit path and audit digest. Recipe
modules and manifests are retained beside those audits.

The polygon recipe uses finite numeric square/edge/vertex/outside/empty samples
and observes added own global keys during calls. The scroll-parent diagnostic
explicitly refuses without a real DOM; it does not emulate one. Neither recipe
passes the transcript capability to package code. **Neither executes**, because
the implementation census refuses first. The absent DOM is an additional
environment limitation inferred from the code, not the measured first refusal.

## Outcomes

| Export | Case | Before this diagnostic | Measured first refusal |
| --- | --- | --- | --- |
| `getScrollParent` | import | withheld; census untested | unknown accessor at `parentNode.parentElement` |
| `getScrollParent` | solid | withheld; census untested | same unknown accessor |
| `isPointInPolygon` | import | withheld; census untested | iteration protocol at `[x, y] = point` |
| `isPointInPolygon` | solid | withheld; census untested | same iteration protocol |

The exact diagnostic suffixes, omitting only the temporary project prefix:

```text
creates census refuses an uncensused invoking form: property-access-unknown-accessor (PropertyAccessExpression) at …/dist/index.js:668..692, reach unknown
creates census refuses an uncensused invoking form: iteration-protocol (ArrayBindingPattern) at …/dist/index.js:2636..2642, reach reachable
```

Claim IDs (each prefixed `claim:v1:sha256:`), in table order:

```text
8b683839c697ba16a5b6567229af0667976a86326d41c193acebc9ae7675295a
12eb2b89dd3ef29e2d7bc8f7b9dd27d029bec21b707cab8385c1e84191a79b29
1a7d8bf9bc3d19515998ab867a309ecb952b88038e2447ab8fd4cf4930001397
1a9735b9711e67a78f3a1b0c640a9cfd51e04b2e53253d216a8c53189c4f26ec
```

Gate IDs derived from each audit's snapshot/demand-graph roots and claim using
`probe_gate_id`'s v2 encoding (each prefixed `sha256:`), in the same order:

```text
b12896c28035cf81c2581ba220fa354dc4306a16f609ab178631bbaf7e6c9f9b
af11ff10309ce4feabacd76ed0198561ea18412968f88fa7beecb0b0c276fdad
83513a0f99d52d21794b4f9948698a86dfd2d4fd95bfbd170abe3942b9fcb0cf
30cc8b00fdcfae06ad2dd971110bdee02765f5b94e21af0d5a5faa301a56efe1
```

All four are **unexecuted**, not completed, contradictory, or `IncompleteGate`.
The distinction matters: this is a missing census proof, not a runtime loading
failure and not a demonstrated package defect. These are first refusals, not
an exhaustive list of every proof obligation that would remain after a fix.

## Control, disposition, and next work

The fifth run uses the existing checked-in corpus against the same JS root.
It certifies both cases with four closures withheld. Both accepted mains still
give `clamp` exactly `closed: ["creates"], creates: []`. Their main digests and
probe roots match ADR 0010's three-row measurement. Thus **two control gates
complete, zero contradictions, zero additional certified closures**. The four
diagnostics complete no gate. `exportsProven` does not advance: only creates
closes, and the accepted mains are unchanged.

The original population remains 43 plus six independent JS candidates, or 49
total and nine structurally loadable. **Zero of the original 43 becomes newly
probeable here.** The 40 source cases remain blocked independently. This
root-only diagnostic is not a replacement three-row benchmark; the last
three-row baseline/comparison remains ADR 0010's recipe-bearing runs.

Keep the checked-in recipe corpus unchanged. The four experimental recipes
remain scratch diagnostics, with their refusals recorded here; no scheduled
gate was waived or reclassified as passing. Consumers of the checked-in
corpus continue to see these four closures withheld and their domains open.
There is no new acceptance policy, sandbox field, or ADR decision to implement.

Unlocking the polygon candidate requires authoritative evidence for the
iterator reached from its untyped JS argument. Unlocking scroll-parent needs
an exact accessor/callee model and a defensible execution environment for DOM
behavior. Reading more `.d.ts` files alone does not establish either runtime
fact, and finite sample success cannot discharge the missing census. These
are separate census/environment slices, outside this workspace slice's scope.
Kobalte 0.9.2's `Aliases` runtime-kind dependency blocker is also unchanged.

Verification for this follow-up: pinned debug build; four isolated real
certifier attempts with demand/refusal identity checks; the successful two-case
control with accepted-main and receipt inspection; and diff whitespace checks.
The earlier full verification is recorded in ADR 0010; it was not repeated for
this documentation-only follow-up. No production source, checked-in recipe,
fixture, snapshot, benchmark or phase20/21 ledger changes; no commit or push.
