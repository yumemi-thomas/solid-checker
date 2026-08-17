package typefacts

import "errors"

// TypeFactsSchemaVersion is the schema stamped on the internal fact table
// materialized inside the producer; the transport model version is
// TypeFactsTableSchemaVersion.
const TypeFactsSchemaVersion uint64 = 1

var ErrGenerationMismatch = errors.New("type facts generation mismatch")

// TypeFactsTableSchemaVersion identifies the fact-table model carried in the
// wire transition and echoed as FactTable.schema by the Rust client.
const TypeFactsTableSchemaVersionV3 uint64 = 3
const TypeFactsTableSchemaVersionV4 uint64 = 4
const TypeFactsTableSchemaVersionV5 uint64 = 5
const TypeFactsTableSchemaVersionV6 uint64 = 6
const TypeFactsTableSchemaVersionV7 uint64 = 7
const TypeFactsTableSchemaVersionV8 uint64 = 8
const TypeFactsTableSchemaVersionV9 uint64 = 9

// v10 is retired rather than frozen: v11 extended the tuple-shape payload it
// introduced instead of adding a new flag, so a v10 row cannot be decoded
// unambiguously. It shipped for one commit and the handshake's digest and
// build-id lockstep make a v10 producer unpairable with any current client.
const TypeFactsTableSchemaVersionV11 uint64 = 11
const TypeFactsTableSchemaVersionV12 uint64 = 12
const TypeFactsTableSchemaVersion uint64 = TypeFactsTableSchemaVersionV12
