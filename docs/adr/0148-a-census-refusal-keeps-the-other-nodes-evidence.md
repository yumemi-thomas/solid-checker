# ADR 0148: A census refusal keeps the other nodes' evidence

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: graph Type Facts acquisition
  (`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`,
  `acquire_and_verify_graph_export_values`, `merge_graph_census_refusals`) and
  the graph lane's recipe-gating loop (`dependencies.rs`,
  `certify_graphs_with_recipe_gating`, `acquire_case_set_evidence`)
- Relation: the second cost cut under the owner's decision of 2026-09-28
  ("make certification cheaper"), after ADR 0144.

## Context

The graph lane acquires every Type Facts node of a case set in one producer
session per pass, and keeps each node's evidence across passes keyed by the
demand-graph root it verified under: a later pass re-acquires only the nodes
whose root moved. That rule had one exception. When *any* node's census
refused, `merge_graph_census_refusals` returned a single `CensusRefused` and
dropped the evidence of every node that had verified in the same session. The
refusing nodes were withheld and the next pass acquired the **whole** graph
again, although only the refusing nodes' roots had moved.

On `@kobalte/core` (160 nodes, 136 cases) the first pass refuses at some nodes,
so the first full acquisition (38 s) was thrown away and repeated (51 s). The
same happened, more cheaply, at every other refusal pass of every graph row.

## Decision

A census refusal no longer discards the other nodes' answers.
`merge_graph_census_refusals` returns the evidence of every node that verified
(`None` for a refusing one) beside the one `CensusRefused` naming every
refusal, and the gating loop stores that evidence under the root it verified
under before it withholds the refusals. Any error other than a census refusal
still fails the whole acquisition, as before.

This is the rule the loop already applied between passes, now applied within a
refusing pass too. A withdrawal moves only the refusing node's demand graph,
so a node that verified keeps its root and is not acquired again; a node whose
root does move is acquired again exactly as before. Nothing about what a node's
evidence is verified against changes: it is still one node's verified answer
for one demand-graph root, from a pinned session over the same authenticated
project.

## Measurement

Host-free `certification-metric` flags, release build, other agents on the
machine (load 6-21):

| | baseline | ADR 0144 | + this ADR |
| --- | ---: | ---: | ---: |
| harness wall, 30 probes | 268 s | 256 s | 187 s |
| summed certification time, 30 rows | 1,665 s | 1,222 s | 1,197 s |
| `@kobalte/core` certification (the corpus tail) | 179 s | 197 s | 122 s |

The headline is unchanged (43 of 957 exports clean; 3.4 % per package, 4.4 %
by downloads). Every row's contract documents and stable receipt roots are
identical to the baseline's, except six `probeGateRoot`s (`@kobalte/core` 5,
`@solidjs/router` 1) that also differ between two runs of the unchanged
baseline binary.

## Tests

`a_census_refusal_keeps_the_evidence_of_every_node_that_verified` pins the
merge: verified answers survive in request order, refusals travel as one
`CensusRefused`, and a non-census error still fails the acquisition.
