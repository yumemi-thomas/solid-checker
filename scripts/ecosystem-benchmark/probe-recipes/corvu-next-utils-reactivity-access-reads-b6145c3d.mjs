// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `./reactivity` runtime case
// `artifact-case:b6145c3d662727593825b1b52488c720ea134ee4696b7582d92a69e820fd8bb8`
// (the corvu-next fork's node 1 row certify through; the chunk
// `dist/chunk/ZV6G25TT.js` is byte-identical to `@corvu/utils@0.4.2`'s, whose
// four recipes this one carries).
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
// NEVER EMITS: `read-operation`. A plain value is returned as is; a callable is invoked, which is the caller's code (ADR 0034).
import { access, chain, mergeRefs, some } from "@corvu-next/utils/reactivity";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  expect(access(3) === 3, "plain value");
  expect(access(() => "x") === "x", "accessor invoked");
  expect(access(undefined) === undefined, "undefined passes through");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
