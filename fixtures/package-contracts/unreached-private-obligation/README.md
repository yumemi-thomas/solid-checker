# Unreached private obligation

`unreached` is private and no export calls it. Exact containment, references,
and reachability therefore attribute its unresolved member dispatch to zero
public exports. `Steady` retains its independently known semantics.

The stable-v1 proposal plan records unresolved semantic subjects and closure
candidates rather than placing attribution notes or inline evidence in the main
document. A zero-export decision must remain visible to the review/proof
workflow, but it must not open `Steady` or synthesize a negative claim for the
private helper. `expected.json` and `expected-proposal.json` pin both sides.
