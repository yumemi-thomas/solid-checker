// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `./create/once` runtime case
// `artifact-case:1b08bdc458c362257be1f8234ada3164218fb64d022631883014272885b28483`
// (16 corpus rows certify through it).
//
// The census decides this closure; a pass-2 scaffold run on 2026-09-14
// confirmed the candidate is not census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. Calling the export only builds and returns a closure over two plain locals; it does not invoke the supplied function and reads nothing.
import { default as createOnce } from "@corvu/utils/create/once";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  let invocations = 0;
  const once = createOnce(() => { invocations += 1; return invocations; });
  expect(typeof once === "function", "a closure is returned");
  expect(invocations === 0, "the supplied function is not invoked at the call event");
  const first = once();
  const second = once();
  expect(first === second, "the memo is built once and reused");
  expect(invocations === 1, "the supplied function ran once");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
