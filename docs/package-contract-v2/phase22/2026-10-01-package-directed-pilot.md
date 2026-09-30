# Package-directed contract pilot (2026-10-01)

Result: **one additional proven misuse finding, no newly complete package**.
Explicit proposals can recover a claim the generator missed, but they cannot
bypass missing proof capabilities. This experiment does not establish a route
to complete certification of every package.

The offline, repeatable experiment is
[`benchmarks/package-directed-pilot/pilot.mjs`](../../../benchmarks/package-directed-pilot/pilot.mjs).
The [extended experiment](2026-10-01-package-directed-extension.md) compares
bounds and hosts, and corrects the initial RAF return proposal's `min: 1`
authoring mistake before identifying its unsupported callable-member leaves.
It changes no analyzer, dialect, schema, receipt policy, accepted tier or misuse
ledger. Generated proposals, trial receipts and observations remain under
`rust/target/package-directed-pilot-checked/`. These are experimental artifacts,
not released contracts. The last measured checkpoint remains 1/97.

## Scope and controls

Three exact published packages were selected for different behaviors, under
**browser** conditions with `solid-js`, `@solidjs/signals` and `@solidjs/web`
all at **2.0.0-rc.9**:

| Package | Target behavior | Runtime SHA256 (`dist/index.js`) |
| --- | --- | --- |
| `@solid-primitives/utils@7.0.0-next.4` | `access`: conditional synchronous argument invocation | `59dd297c1ff4a90620cd3f27aa9b0a650404e8178513d6501af5ec1c3f8370f8` |
| `@solid-primitives/event-listener@3.0.0-next.5` | `createEventListener`: owner-managed effect registration | `15ffa8042705aada967e8ed8a1c6b3f57e1b57c14092888be8d3edefe0b50837` |
| `@solid-primitives/raf@4.0.0-next.2` | `createRAF`: cleanup registration and returned accessor tuple | `5513b328a9174ee6fdb3b8b6790555cafae42994611455bff079c399b796d477` |

Package archive integrity, declaration digests, closure digests and dependency
environment are bound by the existing certification transaction. The pilot
refuses other package or runtime versions. No installs or network fetches are
permitted: missing cached archives stop the experiment.

Each trial generates an initial proposal, changes only the selected export's
summary using existing stable claim forms, and submits those untrusted bytes
to the existing native verifier. Shared summary references are detached before
editing. The reuse envelope's document digest is updated; this grants no proof
authority. The original generator plan remains diagnostic material. Native
execution independently derives demands from the authored document. The
pilot asserts `reusedProposal: true`, receipt authentication and exact-case
admission, so silently regenerating an automatic proposal cannot count as a
successful authored trial.

The positive `utils.access` control preserves both invocation operations,
including the `get` protocol, and conditional `min: 0` bounds. The listener
proposal adds an existing `compute` operation with ambient owner and child-owner
requirements and `min: 1`. The RAF proposal adds an existing ambient cleanup
requirement with `min: 1` and proposes a fixed returned tuple of an accessor and
two callables. Other call domains remain open unless independently proved.

Five existing ledger pairs provide ten consumer examples against the real
published declarations. **All ten pass TypeScript 5.9.3**, strict consumer
checking, renderer-owned JSX and `skipLibCheck` for published declaration-file
defects. Both compiled-tier baseline and trial-catalog analysis run on the same
installed package tree. Checker comparison is refused if TypeScript errors.

## Consumer results

| Pair | Compiled-tier baseline | Authored trial |
| --- | --- | --- |
| `utils.access`, untracked read / JSX | SC1001 **uncertifiable** / clean | Identical |
| `utils.createMicrotask`, module / root | SC4001 **violation** / clean | Identical; unrelated-owner control preserved |
| `event-listener.createEventListener`, module / root | SC9005 + SC4001 **uncertifiable** / SC9005 | SC9005 / SC9005; proposed owner operation withheld |
| `raf.createRAF`, module / root | SC9005 / SC9005 | **SC4001 violation** + SC9005 / SC9005 |
| `raf.createRAF`, eager accessor / JSX | SC9005 / SC9005 | Identical; accessor tuple claim withheld |

The RAF cleanup operation survives into the authenticated main document and
produces a new proven lifecycle finding outside an owner. Its root twin has no
missing-owner finding, but remains uncertifiable because other domains are
open. **It therefore does not pass the complete misuse/correct-use criterion.**

The listener's proposed guaranteed registration is withheld at cardinality
and reachability demands: `owner requirement has no exact dialect primitive
call`, with `createEffect` and `createRenderEffect` present among the observed
calls. This message alone does not establish failed symbol resolution:
`require_owner_operation_call` also filters calls by the demanded reachability
floor. Its mutually exclusive registration branches require investigation
before choosing a proof extension. The weakened trial also loses the baseline
SC4001 uncertifiable finding; it is not a replacement for the delivered tier.

The RAF return operation is withheld at `domain-exhaustiveness` with
`structural closure output is absent or not a bare return`. The emitted main
retains only the proposed cleanup operation for this target. Receipt issuance
for this partial document must never be reported as acceptance of its proposed
tuple or certification of the whole package.
The later extension establishes that this initial refusal was caused by the
proposal's return bound; its corrected conditional return reaches a separate
unsupported-member refusal. Do not cite the initial message alone as evidence
of a returned-value proof gap.

## Negative control

A fourth trial removes `utils.access`'s argument getter invocation while
requesting the same closed callback domain. The verifier explicitly withholds
`callbacks`; the authenticated main leaves it open. The source evaluates
`v.length`, which can execute caller-supplied code even when `v` is a function.
An additional direct invocation of the actual published `access` with an
overridden function `length` getter returned 42 and invoked that getter once.
Thus omitting this invocation is a real incomplete contract, not harmless
simplification. Passing a handwritten claim into the verifier did not make it
trusted.

## Decision

Keep package-directed proposals as a diagnostic and authoring layer. Use them
to distinguish a missing proposal from a missing proof before changing the
generator. The useful RAF owner claim demonstrates the former; listener
registration and RAF returned values demonstrate remaining proof limits.

The next bounded investigation should address those existing claim forms,
with exact symbols, branch/completion evidence and clean published consumers.
There is no evidence here that runtime samples or handwritten descriptions can
replace source proofs. New claim forms or value shapes still require owner
approval. No whole-package or three-host completion is claimed.

Focused validation consists of the four certification trials, authenticated
accepted-document inspection, twenty checker observations, ten TypeScript
checks and the published getter falsifier. The runner asserts the positive
controls, RAF owner improvement, refusal of its unsupported return, and the
incomplete getter enumeration. Full verification, census, tier regeneration,
app sweeps and three-host checkpoint runs are deferred: no product semantics
or accepted artifacts changed. Universal formatting, Clippy, schema, manifest
and diff checks are recorded in the handoff.
