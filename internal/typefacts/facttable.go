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
const TypeFactsTableSchemaVersion uint64 = TypeFactsTableSchemaVersionV6
