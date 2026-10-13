// Hand-authored `reads: []` veto for `@corvu/utils@0.3.2`,
// on the published `./dom` runtime case
// `artifact-case:84d6a8cd6af86d14ff90de35550d57a163b19ad8eec05d481a9580502fd55911`
// (the 0.3.2 node three rows certify through; the same two bodies as 0.4.2).
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
// NEVER EMITS: `read-operation`. A function handler is called; a bound-handler tuple is called with its data; the returned flag is the event's own `defaultPrevented`.
import { callEventHandler } from "@corvu/utils/dom";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const seen = [];
  const event = { defaultPrevented: false };
  expect(callEventHandler((e) => seen.push(e), event) === false, "function handler");
  expect(callEventHandler([(data, e) => seen.push([data, e]), 7], event) === false, "bound handler tuple");
  expect(callEventHandler(undefined, { defaultPrevented: true }) === true, "no handler returns the event flag");
  expect(seen.length === 2 && seen[0] === event && seen[1][0] === 7, "handlers received the event");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
