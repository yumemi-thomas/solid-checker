package typefacts

import "testing"

// ApplyFactTableDeltaV3ForTest exposes the delta applicator model to the
// external test package, which cannot live in package typefacts because it
// imports the tsgo backend (which imports typefacts).
//
// The model mirrors apply_table_delta in crates/typefacts/src/session.rs and
// enforces the same invariants, so a delta the Rust client would reject fails
// here too. See protocolv3_delta_oracle_test.go.
func ApplyFactTableDeltaV3ForTest(t *testing.T, previous FactTableV2, delta FactTableDeltaV3) FactTableV2 {
	t.Helper()
	return applyFactTableDeltaV3(t, previous, delta)
}
