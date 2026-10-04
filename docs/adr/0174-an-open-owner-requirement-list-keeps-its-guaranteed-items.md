# ADR 0174: an open owner-requirement list keeps its guaranteed items

Date: 2026-10-04. Status: implemented. No contract or Type Facts schema
change; the accepted tier is regenerated for the browser host.

## Context

The generator publishes an export's owner requirements as `cleanups` and
`computations` items (ADR 0114). Each item is a dialect primitive call in the
export's own body. ADR 0161 adds a `min: 1` lower bound when the call is made on
every normal completion, and the certifier's census proves that bound or
withdraws it. A consumer reads a `min: 1` item as "every call registers": an
unowned call is then a proven `missing-owner` violation, and without the bound
it is only uncertifiable.

Two generation paths discarded proven items before they reached the document:

- **Unresolved obligations.** An obligation whose domains include
  `ownerRequirements` sets the export's list to `Open`, for example an import
  whose dependency contract does not describe the binding
  (`PackageContractExportMissing`). `Open` carried no items, so the generator
  published neither `cleanups` nor `computations`. Opening the list says
  another item may exist. It does not disprove an item the body makes on every
  call: that item comes from the export's own direct call, which the obligation
  does not concern.
- **Runtime aliases.** `unify_runtime_alias_summaries` gives every name of one
  runtime function a single summary. It rebuilt that summary from
  `ContractExport::default()`, merging reads, callbacks, returns and async
  behaviour but not owner requirements. The default is `Known(vec![])`, so
  every alias lost its items, and the empty list read as "this export requires
  no owner".

The [owner-claims research](../package-contract-v2/phase22/2026-10-04-precision-and-misuse-evidence.md)
traced five `@solid-primitives` misuse-ledger cases with no static finding to
these two paths.

## Decision

1. `ContractExport::open_owner_requirements` holds the guaranteed items still
   proven while `owner_requirements` is `Open`. It is generation-only state: it
   is never decoded from or encoded into a package-contract document, and it is
   empty whenever the list is `Known`.
2. Opening the list (`mark_summary_claims_unknown`) moves the list's guaranteed
   items into that field. A possible (`guaranteed: false`) item is dropped. A
   consumer turns it into a proof obligation, and the open list already leaves
   one, so publishing it would only add uncertifiable findings.
3. When the summary is already `Open`, `attach_generated_owner_requirements`
   adds the export's own guaranteed requirements to the field instead of
   discarding them.
4. `unify_runtime_alias_summaries` merges owner requirements. Every name is
   the same function, so an item proven under one name holds for all of them.
   The merged list is `Known` only when every name's list is; otherwise it is
   `Open` and keeps the guaranteed items of every name.
5. The emitter publishes an open list's items exactly as it publishes a known
   list's, as `cleanups` or `computations` operations with their lower bound.
   `requirements_published` stays `false`, so `creates` is never closed over an
   open list. A consumer reads a closed `creates` as "no owner requirement
   beyond the published items" (`project_owner_requirements`), and over an
   incomplete list that reading would be false.

## Consequences

- No item is invented. Each one is a site the owner census already found and
  the certifier still proves or withdraws by name. What changes is that a
  proven item survives an unrelated obligation and the alias merge.
- `creates` stays open for every export these paths touch, so a consumer still
  cannot conclude an absence from such a contract.
- The bundle generator gains `--carry`
  (`scripts/bundle-accepted-contracts.mjs`). It regenerates part of the tier
  without the runs that produced the rest: a bundle whose exact key a new run
  certified is replaced, and every other bundle is carried byte for byte. The
  checked-in tier round-trips through it unchanged. Citation withdrawal
  (ADR 0151) still runs over the merged set.
- `makeEventListener` and `tryOnCleanup` gain nothing, deliberately. Their
  registrations go through `tryOnCleanup`, which tolerates a missing owner, so
  no `min: 1` cleanup item reaches the caller, and the runtime is silent on the
  same cases.

## Evidence

- `fixtures/package-contracts/open-owner-requirements`:
  - an export whose list the refused dependency contract opens keeps its
    `min: 1` cleanup;
  - a conditionally registering sibling publishes nothing;
  - both names of one aliased function keep the item.
- Unit tests:
  - `open_owner_requirement_tests` (marking, and the alias merge, known and
    open);
  - `an_open_owner_requirement_list_publishes_its_items_and_keeps_creates_open`
    (the emitter, including a clean `creates` walk that must not close).
- Measurement
  ([`2026-10-04-open-owner-requirements.md`](../package-contract-v2/phase22/2026-10-04-open-owner-requirements.md)),
  with the browser tier regenerated:
  - The ledger's static violations go from 45 to 49 of 123. The four new
    ones are `createElementSize`, `createRAF`, `createFullscreen` and
    `createRootPool`, and the runtime detects each of them.
  - No correct twin is flagged, and the 48-project sweep adds nothing.
  - `createSwitchTransition` is proposed but withdrawn by the census, the
    fail-closed outcome.
