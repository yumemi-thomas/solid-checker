// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `./create/keyedContext` runtime case
// `artifact-case:1116a60f342774885e13dcf6b64661f297fc0d332f6b68a5c56b235ea2759c20`
// (16 corpus rows certify through it).
//
// The census decides this closure; a pass-2 scaffold run on 2026-09-14
// confirmed the candidate is not census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. The export performs one `Map.get` on a module-local registry this package owns; a `Map` is not a reactive source.
import { createKeyedContext, getKeyedContext, useKeyedContext } from "@corvu/utils/create/keyedContext";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const key = "solid-checker-probe:get:1116a60f";
  expect(getKeyedContext(key) === undefined, "an unregistered key reads back undefined");
  const context = createKeyedContext(key, 1);
  expect(getKeyedContext(key) === context, "a registered key reads back the same context");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
