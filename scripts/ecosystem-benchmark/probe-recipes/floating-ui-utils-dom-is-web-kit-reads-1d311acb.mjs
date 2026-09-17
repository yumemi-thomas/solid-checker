// Hand-authored `reads: []` veto for `@floating-ui/utils@0.2.12`,
// on the published `./dom` runtime case
// `artifact-case:1d311acb3467d439f2a09329da1770216a63d44466c1036ea199b14f65b252cd`
// (the node `@corvu-next/popover@0.1.5`'s 8 rows certify through).
//
// What this export reads is its caller's: every value it inspects is reached
// off an argument the caller passed, so under ADR 0034 the read belongs to the
// caller's code and not to this package. That is what the census proves; this
// module is the mandatory veto beside it, and its job is to call the export
// and emit only on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. The export reads the host `CSS` global once and memoizes the answer in a module-local binding; neither is a reactive source this package owns.
import { getContainingBlock, getFrameElement, getNearestOverflowAncestor, getNodeScroll, getWindow, isLastTraversableNode, isTableElement, isWebKit } from "@floating-ui/utils/dom";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  // No `CSS` in this realm, so the feature test short-circuits and memoizes false.
  expect(isWebKit() === false, "absent CSS support test answers false");
  expect(isWebKit() === false, "the memoized answer is stable");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
