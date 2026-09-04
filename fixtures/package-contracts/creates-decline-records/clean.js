// The control artifact case: the walk runs, clears an export, and a
// `creates: []` candidate is proposed.
//
// It is a **separate entrypoint** on purpose. `index.js` imports `solid-js`,
// and a bare specifier that resolves to no accepted dependency is an
// `UnacceptedExternalDependency` closure hazard that opens every domain of
// that artifact case at closure replay — so no candidate can survive there
// however clean the walk was. This module imports nothing, so its closure
// carries no hazard and the proposal is visible in `expected-proposal.json`.
// Keeping the control in the same module as the declines would have made
// "nothing proposed" ambiguous between the walk and the hazard.

// `callback(0)` is a caller-supplied callable: not a canonical primitive, not
// a contracted dependency export, and resolved — no counterexample the walk
// can name.
export function proposes(callback) {
  callback(0);
}
