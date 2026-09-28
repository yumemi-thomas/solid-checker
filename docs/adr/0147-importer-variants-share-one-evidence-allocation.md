# ADR 0147: Importer variants share one evidence allocation

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: graph Type Facts acquisition
  (`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`,
  `GraphExportValues`) and the graph lane's evidence map (`dependencies.rs`,
  `certify_graphs_with_recipe_gating`)
- Relation: the memory half of the owner's decision of 2026-09-28 ("make
  certification cheaper"); measures ADR 0073's resource bound. ADR 0073 is not
  amended: its 32-case graph budget stays.

## Context

A published-graph case set holds one graph node per *importer* of a package.
Graph Type Facts acquisition already acquires importer variants once (their
demand graphs are equal) and hands the representative's evidence to every
variant, but it handed each variant a deep copy
(`evidence[*position].clone()`), and the gating loop kept every copy for the
rest of the transaction.

Measured with the timings' `peakRssMiB` stage lines on `@kobalte/core` with its
132 retained roots given dependency graphs (the recovery case budget raised
locally from 32; the experiment ADR 0073's bound refuses): the case set expands
to 136 graphs, 3,402 node references and 582 canonical nodes; planning peaks at
1.8 GB, and every recipe-gating pass then adds about a gigabyte as more
variants acquire, to 13.3 GB before the gate pass and 14.3 GB at finalization.
The memory sits in retained evidence copies, not in planning or the producer
(`solid-typefacts` stays under 700 MB).

## Decision

One acquisition's evidence is one `Arc<VerifiedTypeFactsEvidence>`, shared by
every importer variant it answers and by the gating loop's evidence map. The
evidence is immutable after verification, and every reader takes it by
reference, so nothing reads anything different.

## Measurement

The same raised-budget experiment, release build: the native transaction's
peak falls from 14,264 MiB to 5,515 MiB (process tree 16.9 GB to 8.0 GB; the
rest is the Node-side preparation), and the row certifies all 136 cases in
969 s (2,585 s before ADR 0144). Every contract document is identical; 13
`probeGateRoot`s differ, and the same 13 differ between two runs without this
change.

Not solved: the tree still exceeds the benchmark's 4,096 MiB ceiling (the
Node side alone peaks at 2.5 GB, and the native planning of 3,402 node
references at 1.8 GB), so the 32-case budget stays and `@kobalte/core`'s
retained roots still certify without dependency contracts. The next costs are
the per-case expansion of shared nodes (3,402 references for 582 nodes) and
the second-wave gate workspaces.
