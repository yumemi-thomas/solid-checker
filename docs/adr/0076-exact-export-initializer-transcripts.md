# Exact export initializer transcripts

Status: producer and session boundary implemented. The subsequent
[ADR 0077](0077-receipt-bound-factory-export-roots.md) implements the conditional
certification consumer and receipt discharge.

Protocol 44 adds the optional `initializer` field to an export-value
transcript. It answers the exact runtime implementation location, independently
of whether the declared export has a unique call signature. An `any` export
remains unknown: this field is a source derivation, not a result-shape verdict.

The producer resolves the query's exact compiler symbol, then follows at most
16 positively unwritten, single-declaration variable bindings within that
runtime module. Each binding records its declaration, full source span and
initializer span. Forward aliases, cycles, redeclarations, assignments,
eval/arguments exposure, nested bindings, optional calls and spread arguments
remain unsupported. A query may import the runtime binding from a separate
harness; this does not permit the initializer chain to cross modules.

The terminal ordinary call records its callee expression, exact resolved
declaration, import specifier and export name. Direct object-literal arguments
are recorded in source order with their actual argument indexes. Neither
properties nor factory behavior are classified.

The Rust session binds the row to the demanded runtime location and resolved
implementation target, requires source-envelope coverage of the query, runtime
module and callee declaration, and validates bounded unique binding identities,
containment, source order, terminal call identity and distinct ordered argument
slots. Missing rows grant no premise. Malformed positive rows are refused.

This is the transport prerequisite for the
[factory composition design](../package-contract-v2/phase21/2026-09-08-factory-composition-design.md).
The certification consumer must still authenticate the dependency's exact
closed whole-parameter return and receipt in the current graph context, and
finalization must discharge and commit that conditional positive proof. No
export-root refusal is cleared by this change, and no new artifact case or
complete-row transition is claimed.

Validation: the producer's focused initializer tests passed, including an
imported runtime query with an `any` declared value, source-ordered argument
slots 0 and 2, CBOR round-trip preservation and an unrequested-demand negative
control. The Rust focused test passed 21 malformed-row controls plus missing
source, missing runtime transcript and unrequested-demand controls. Full
`make verify` passed with actual exit 0, `TOTAL 160.97s`, and no
`FAILED during step` marker in
`/private/tmp/export-initializer-protocol-verify.log`. The workflow rebuilt
matching producer and verifier pins. Existing snapshots were unchanged by this
slice. The full ecosystem corpus has not been rerun; its last measured totals
remain the protocol-43 retained-graphs report, not a protocol-44 measurement.
