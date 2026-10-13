// Hand-authored `reads: []` veto for `@corvu/utils@0.3.2`,
// on the published `./reactivity` runtime case
// `artifact-case:f240869a5d572042677786a7ad688dc6f56939e7e40258e369a3aad6ccbf9992`
// (the 0.3.2 node three rows certify through; the same four bodies as 0.4.2).
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that `dist/reactivity/index.js` installs no accessor and there is no trap
// here for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// The hazard does the work an observation counter used to. What is left is
// what a mandatory veto is for: sample the export and emit only on
// contradiction.
//
// NEVER EMITS: `read-operation`. Each supplied signal is invoked until one is truthy; those invocations are the caller's code (ADR 0034).
import { access, chain, mergeRefs, some } from "@corvu/utils/reactivity";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let calls = 0;
  const off = () => { calls += 1; return false; };
  const on = () => { calls += 1; return true; };
  expect(some(off, on, off) === true && calls === 2, "stops at the first truthy signal");
  expect(some(off, off) === false, "false when none is truthy");
  expect(some() === false, "no signals");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
