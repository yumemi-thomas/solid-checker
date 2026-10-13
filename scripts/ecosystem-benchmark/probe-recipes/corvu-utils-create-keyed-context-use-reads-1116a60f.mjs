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
// NEVER EMITS: `read-operation`. The export reads a module-local `Map` this package owns and then calls `solid-js`'s `useContext`; the context it observes is `solid-js`'s source, not one this package owns.
import { createKeyedContext, getKeyedContext, useKeyedContext } from "@corvu/utils/create/keyedContext";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const key = "solid-checker-probe:use:1116a60f";
  expect(useKeyedContext(key) === undefined, "an unregistered key yields undefined without reaching useContext");
  createKeyedContext(key, 7);
  expect(useKeyedContext(key) === 7, "a registered key yields the default outside a provider");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
