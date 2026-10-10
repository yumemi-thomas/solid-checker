// Hand-authored `reads: []` veto for `@solid-primitives/scheduled@1.5.3`,
// on the published runtime case
// `artifact-case:e1a524faf8d0e2a0d73375009862d94f64a597d298c287007a6628538ee37484`
// (4 corpus rows certify through it).
//
// The census decides this closure; the 2026-09-14 pass-2 census found this
// export decidable rather than census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. At its call event the export builds a signal and invokes the scheduler its caller supplied; building a source is not reading one, and the returned accessor's reads happen at the caller's invocation.
import { createScheduled, debounce, leading } from "@solid-primitives/scheduled";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let scheduled = 0;
  const invalidate = createScheduled((notify) => { scheduled += 1; return () => {}; });
  expect(typeof invalidate === "function", "an accessor is returned");
  expect(scheduled === 1, "the caller's scheduler is invoked once at the call event");
  expect(invalidate() === false, "untracked and not yet dirty, the accessor reports false");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
