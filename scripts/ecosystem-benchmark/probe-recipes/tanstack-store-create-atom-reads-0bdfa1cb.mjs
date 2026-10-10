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
// NEVER EMITS: `read-operation`. Constructs an atom around a value or a computation; the construction itself reads nothing, and a later `get` is the caller's.
import { batch, createAtom, createAsyncAtom, createStore } from "@tanstack/store";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const value = createAtom(1);
  expect(typeof value.get === "function" && typeof value.subscribe === "function", "an atom has get and subscribe");
  const computed = createAtom(() => 2);
  expect(typeof computed.get === "function", "a computed atom has get");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
