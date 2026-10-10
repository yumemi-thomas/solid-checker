// Hand-authored `reads: []` veto for `@solid-primitives/scheduled@2.0.0-next.2`,
// on the published runtime case
// `artifact-case:6f867f0c52fcf2326754d14e3732fc846506169f421964acdd213ca859cbbd8d`
// (2 corpus rows certify through it).
//
// The census decides this closure; the 2026-09-14 pass-2 census found this
// export decidable rather than census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. The export invokes the scheduler its caller supplied and closes over one plain boolean; neither is a read of a source this package owns.
import { createScheduled, debounce, leading } from "@solid-primitives/scheduled";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let invocations = 0;
  const scheduler = (callback, wait) => Object.assign(() => {}, { clear: () => {} });
  const triggered = leading(scheduler, () => { invocations += 1; }, 5);
  expect(typeof triggered === "function", "a callable is returned");
  expect(typeof triggered.clear === "function", "the callable carries a clear");
  expect(invocations === 0, "the supplied callback is not invoked at the call event");
  triggered();
  triggered();
  expect(invocations === 1, "the callback runs once, on the leading edge");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
