// Hand-authored `reads: []` veto for `@tanstack/store@0.11.1`,
// on the published `.` runtime case
// `artifact-case:0bdfa1cbfcd08632d3def1207aa3b0c24966e3093cf9cb6fa9ce35b1d52fce0d`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// NEVER EMITS: `read-operation`. The supplied function runs once, synchronously, inside the batch; the export reads no atom itself.
import { batch, createAtom, createAsyncAtom, createStore } from "@tanstack/store";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let ran = 0;
  batch(() => { ran += 1; });
  expect(ran === 1, "batch runs its function once");
  let threw = false;
  try { batch(() => { throw new Error("inner"); }); } catch { threw = true; }
  expect(threw, "batch rethrows and still ends the batch");
  batch(() => { ran += 1; });
  expect(ran === 2, "a later batch runs after the throwing one closed");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
