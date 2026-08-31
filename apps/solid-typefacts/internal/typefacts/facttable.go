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
const TypeFactsTableSchemaVersionV13 uint64 = 13
const TypeFactsTableSchemaVersionV14 uint64 = 14

// v15 changes no row layout: it widens callability's closed tag space by one
// value (untypedCallable). A v14 decoder rejects that tag rather than guessing,
// so the version — not the flag set — is what tells a reader which vocabulary
// the tags in front of it come from. Emission at v14 or earlier degrades the
// new value to unknown so those frozen schemas stay exactly decodable.
const TypeFactsTableSchemaVersionV15 uint64 = 15
const TypeFactsTableSchemaVersionV16 uint64 = 16
const TypeFactsTableSchemaVersionV17 uint64 = 17
const TypeFactsTableSchemaVersionV18 uint64 = 18
const TypeFactsTableSchemaVersion uint64 = TypeFactsTableSchemaVersionV18
