# Retain proposal roots when recovering a large graph case set

The recovery lane previously prepared semantic dependency graphs for every
generated case plus the missing dependency frontier. Kobalte Core 2 has 118
generated artifact cases across 59 entrypoints and four frontier cases. That
expansion exceeded the benchmark's 4,096 MiB process-tree ceiling, so recovery
was subsequently refused above 32 total cases. The refusal preserved existing
coverage but also excluded a small frontier solely because its retained set
was large.

For sets above 32 cases, retain the original unaccepted proposals as independent
graph roots with their ordinary authenticated compiler-source inputs. Prepare
semantic dependency graphs only for the missing frontier, still bounded to 32
cases. The full publication census remains bounded to 1,024 cases. Smaller
sets keep their measured graph preparation strategy.

This uses the existing schema-5 graph case-set request. Each retained root names
its original proposal, importer, resolution conditions, registry archive and
metadata, exact lock selection and its own compiler sources. It receives no
semantic child receipts from the caller. Native planning independently derives
all demands and refuses any missing dependency. The fresh final transaction
certifies every selected root, composes dependencies in their own graphs, and
verifies ordinary consumption before publishing the combined set. No earlier
receipt or private trial result becomes proof input.

The retained baseline is mandatory for this strategy. If the complete set
refuses, a private transaction must prove all retained roots before trying the
frontier cases. A retained-baseline refusal abandons this strategy and runs the
existing proposal fallback; it never authorizes dropping retained roots to
make the graph pass. Successful trials only select whole cases, and the final
union is freshly verified. Duplicate selections, conflicting coordinates,
incomplete compiler-source vectors and final verification failures refuse.

The implementation lives in `retained-proposal-graphs.mjs` and the certification
orchestrator. Protocol, receipt, trust and native certification interfaces are
unchanged. Seven focused tests pin bounds, exact retained inputs, preservation,
explicit frontier refusals and final-union failure. The complete CLI suite
passes 207 tests and its TypeScript check.

The first native Kobalte experiment publishes 122 cases in one transaction,
up from 118: `./colors` and `./i18n`, each under default and `solid` conditions.
The [exact measurement](../package-contract-v2/phase21/2026-09-08-retained-graphs-kobalte-measurement.json)
records all before/after selections and receipt bindings. No prior selection
or claim is removed; two `createRegisterId` cases additionally close `creates`
through the existing verifier. Those incidental strengthened claims are not
entrypoint gains. Wildcard completeness remains unestablished.

A Solid 1 diagnostic reaches the graph verifier but encounters an unproved
`seroval-plugins/AbortSignalPlugin` export shape and an unproved retained
`createResource` tuple path. Its proposal fallback remains active. This change
does not grant either premise or assert that every budget-limited row improves.
The full 418-probe rerun confirms 1,492 → 1,496 artifact selections with every
prior selection preserved. Counts remain 324 complete, 62 partial, 23 refused
and 9 not advanced. Exact inventories and claim changes are recorded in the
[recovery report](../package-contract-v2/phase21/2026-09-08-retained-graphs-recovery.md).
