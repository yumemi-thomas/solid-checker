// `createTrackedEffect` is declared *locally*, exactly as `@solidjs/signals`'
// own published bundle declares it. No import establishes provenance, so the
// only thing that makes this call a dialect primitive is
// `declaration_path_is_solid_package`
// (rust/crates/solid-reactive-ir/src/symbols.rs) matching the `@solidjs` path
// component this file sits under. That is the path bootstrap, and it exists so
// the checker can analyze Solid's own repository.
function createTrackedEffect(compute: () => void, options?: { name?: string }): void {
  compute();
  void options;
}

// The shape of `@solidjs/signals@2.0.0-rc.3`'s `onSettled`, reduced to the one
// arm that matters. `find_missing_owners` reads the `createTrackedEffect` call
// as a create needing an ambient owner with child-owner capability, and
// `apply_owner_requirement` publishes that as an `owner-requirement-0` create
// on this export -- a consumer obligation invented from the primitive's own
// implementation. The audited bundled contract for the real bytes
// (pkg/contracts/bundled/solid-v2/solidjs-signals.json) closes `creates` as
// empty for this export, so the generated claim contradicts the audit.
export function onSettled(callback: () => void): void {
  createTrackedEffect(() => callback(), { name: "onSettled" });
}
