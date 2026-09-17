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
// NEVER EMITS: `read-operation`. Same walk as `getFirstChild`, collecting every match instead of the first; the predicate and the value walked are both the caller's (ADR 0034).
import { defaultElementPredicate, getFirstChild, getResolvedElements, resolveFirst } from "@solid-primitives/refs";
import { createRoot } from "solid-js";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const isElement = (value) => !!value && value.__element === true;
  const resolved = getResolvedElements(
    [{ __element: true, id: 1 }, [{ __element: true, id: 2 }]],
    isElement
  );
  expect(Array.isArray(resolved) && resolved.length === 2, "nested matches are flattened");
  expect(resolved[0].id === 1 && resolved[1].id === 2, "document order is preserved");
  expect(getResolvedElements([1, 2], isElement) === null, "no match yields null");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
