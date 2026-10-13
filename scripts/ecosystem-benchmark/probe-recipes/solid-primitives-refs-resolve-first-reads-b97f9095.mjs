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
// NEVER EMITS: `read-operation`. The export builds two memos which compute at the call event; what they invoke is the caller's `fn` and the caller's predicate, so the reads are the caller's (ADR 0034).
import { defaultElementPredicate, getFirstChild, getResolvedElements, resolveFirst } from "@solid-primitives/refs";
import { createRoot } from "solid-js";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const isElement = (value) => !!value && value.__element === true;
  let invocations = 0;
  createRoot((dispose) => {
    const first = resolveFirst(
      () => { invocations += 1; return [{ __element: true, id: 9 }]; },
      isElement
    );
    expect(first().id === 9, "the first matching element is resolved");
    expect(invocations >= 1, "the caller's children accessor was invoked");
    dispose();
  });
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
