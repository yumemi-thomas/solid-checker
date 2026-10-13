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
// NEVER EMITS: `read-operation`. At its call event the export builds a closure and, on the client branch, registers a cleanup with `solid-js`; it reads no source of its own.
import { createScheduled, debounce, leading } from "@solid-primitives/scheduled";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let invocations = 0;
  const debounced = debounce(() => { invocations += 1; }, 5);
  expect(typeof debounced === "function", "a callable is returned");
  expect(typeof debounced.clear === "function", "the callable carries a clear");
  expect(invocations === 0, "the supplied callback is not invoked at the call event");
  debounced.clear();
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
