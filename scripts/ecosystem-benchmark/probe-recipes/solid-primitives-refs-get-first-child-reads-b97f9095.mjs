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
// NEVER EMITS: `read-operation`. The export invokes the predicate its caller supplied and walks the value its caller supplied, including calling a zero-arity thunk of the caller's; every value it inspects is the caller's (ADR 0034).
import { defaultElementPredicate, getFirstChild, getResolvedElements, resolveFirst } from "@solid-primitives/refs";
import { createRoot } from "solid-js";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const isElement = (value) => !!value && value.__element === true;
  expect(getFirstChild([[{ __element: true, id: 1 }], { __element: true, id: 2 }], isElement).id === 1,
    "the first match wins, depth first");
  expect(getFirstChild(() => ({ __element: true, id: 3 }), isElement).id === 3,
    "a zero-arity thunk is invoked and its result resolved");
  expect(getFirstChild([1, "x", null], isElement) === null, "no match yields null");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
