// Hand-authored `reads: []` veto for `@solid-primitives/refs@1.1.4`,
// on the published runtime case
// `artifact-case:b97f9095b2c226f35e2cbcc4d0edd6ff54b8c0650d4994a443e7a3e38403497c`
// (6 corpus rows certify through it).
//
// The census decides this closure; the 2026-09-14 pass-2 census found this
// export decidable rather than census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. The export applies one `instanceof` test to the value its caller supplied; a prototype-chain walk is not a read of a reactive source, and the value is the caller's own (ADR 0034).
import { defaultElementPredicate, getFirstChild, getResolvedElements, resolveFirst } from "@solid-primitives/refs";
import { createRoot } from "solid-js";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  // The client branch tests `item instanceof Element`, and the harness realm
  // has no DOM. A stand-in constructor is enough for the branch to run.
  class ProbeElement {}
  globalThis.Element = ProbeElement;
  expect(defaultElementPredicate(new ProbeElement()) === true, "an Element instance matches");
  expect(defaultElementPredicate({}) === false, "a plain object does not match");
  expect(defaultElementPredicate(null) === false, "null does not match");
  expect(defaultElementPredicate("s") === false, "a primitive does not match");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
