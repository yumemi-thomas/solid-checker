package typefacts

import "errors"

// TypeFactsSchemaVersion is the schema stamped on the internal fact table
// materialized inside the producer; the transport model version is
// TypeFactsTableSchemaVersion.
const TypeFactsSchemaVersion uint64 = 1

var ErrGenerationMismatch = errors.New("type facts generation mismatch")

// TypeFactsTableSchemaVersion identifies the fact-table model carried in the
// wire transition and echoed as FactTable.schema by the Rust client.
const TypeFactsTableSchemaVersion uint64 = 3
