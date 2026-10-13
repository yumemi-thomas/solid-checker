// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `./dom` runtime case
// `artifact-case:fd42a1d9dda954dae8c911fad3e828d82b47a4e85f5447e264836f53e6557e5e`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/dom/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// NEVER EMITS: `read-operation`. The export schedules its callback through two animation frames and reads nothing itself.
import { afterPaint } from "@corvu/utils/dom";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  // The harness realm has no paint loop; a same-turn frame stands in for it so
  // the sample can complete, and the callback records that it ran.
  let ran = 0;
  globalThis.requestAnimationFrame = (frame) => { queueMicrotask(() => frame(0)); return 1; };
  afterPaint(() => { ran += 1; });
  await Promise.resolve(); await Promise.resolve();
  expect(ran === 1, "afterPaint runs its callback once after two frames");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
