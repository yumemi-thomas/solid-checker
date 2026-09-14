// Hand-authored `reads: []` veto for `@corvu-next/utils@0.1.5`,
// on the published `./dom` runtime case
// `artifact-case:3c29eec4eb9196a19474b6bcdb91a44faf4ce109978d1a761e530e13bf585b96`.
//
// Why this recipe is allowed to be this short: the artifact case carries no
// `runtime-accessor-installation` closure hazard, so the census states
// syntactically that the module installs no accessor and there is no trap here
// for the recipe to count (§ 12 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
// What is left is what a mandatory veto is for: sample the export and emit only
// on contradiction.
//
// NEVER EMITS: `read-operation`. A function handler is called; a bound-handler tuple is called with its data; the returned flag is the event's own `defaultPrevented`.
import { callEventHandler } from "@corvu-next/utils/dom";
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
