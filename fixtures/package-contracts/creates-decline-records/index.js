// Why the generator's own `creates` walk declined to propose, one export per
// blocker kind.
//
// This is the *measurement* fixture for `CreatesProposalWalk`, not another
// census fixture: nothing here is certified, and what it pins is the
// `declinedClosures` array of the proposal refusal sidecar — the record that
// says which blocker stopped a `creates: []` proposal from being made at all.
// See README.md for what each export is for and, importantly, for what
// `dialect-silent` does and does not mean here. The export that still
// *proposes* is `./clean`, in its own module: this one's top-level
// `import "solid-js"` is a closure hazard that opens every domain of this
// artifact case regardless of the walk, so a control here would prove nothing.
import { createEffect } from "solid-js";

// `createEffect` is canonical Solid 2.0 vocabulary and no dialect's audited
// negative authority carries a `creates` denial row for that spelling
// (`rust/crates/solid-dialect/src/solid_2.rs`: the row was *withdrawn*
// 2026-09-04). Silence is "do not propose", so this declines with
// `dialect-silent`, naming the resolved package and the canonical spelling —
// which is exactly the pair `scripts/dialect-audit-yield.mjs` ranks.
export function dialectSilent(value) {
  createEffect(
    () => value,
    () => {}
  );
}

// Byte-identical body, one call away. The walk is lexical, so nothing inside
// `viaSilentHelper` refuses on its own; the fixpoint over the resolved local
// call edge is what refuses the call into this helper. The export therefore
// declines twice: `refusing-callee-fixpoint`, naming the helper's exact
// declaration span, *and* the helper's own `dialect-silent` record, kept at
// its own location inside the helper. Reporting only the first would make the
// primitive invisible in the ranking on exactly the shape real consumer
// packages have.
function silentHelper(value) {
  createEffect(
    () => value,
    () => {}
  );
}

export function viaSilentHelper(value) {
  silentHelper(value);
}

// An identifier nothing declares, which this build resolves to no symbol at
// all. "Unresolved" is never evidence of harmlessness, so the walk declines —
// and the record carries the call's location and no callee identity, because
// there is none to carry. Deliberately a bare global rather than an import
// from an unaudited dependency: that would be a closure hazard decided
// elsewhere, and no walk decision at all.
export function unresolvedCallee(value) {
  return externalGlobal(value);
}
