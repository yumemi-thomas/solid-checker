# ADR 0236: Solid Primitives batch 3

- Status: accepted and implemented (2026-10-08), in slices (3a, 3b, 3c).
- Owners: `pkg/contracts/authored/specs/@solid-primitives+*` (the new version
  specs), their probe pairs.
- Relation: the third batch under ADRs 0198, 0226 and 0227, drafted read-only
  by research agents and admitted only through the authoring tool and Chrome
  probes on rc.13.

## Slice 3a

- Specs: `a11y`, `active-element`, `clipboard`, `connectivity`,
  `fullscreen`, `page-utilities`, `pagination`, `upload` and `websocket`.
  `storage` gets no claim: its forwarded getter's identity cannot be
  projected yet.
- 29 probe pairs pass in Chrome. Two claims do not ship, because a claim
  ships only when all its pairs pass: `createPagination` (its constructor
  read warns unattributed) and `upload`'s `createDropzone` (harness errors).
- Primitives ledger, browser: 26 report correctly (was 21). Seven more prove
  the misuse while their correct twin keeps an open domain. No correct twin
  on any host has a violation.
