// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `./create/controllableSignal` runtime case
// `artifact-case:0702583fdcc186419c72816af85162a2f3b5244131b76f97c2823b215f616798`
// (18 corpus rows certify through it).
//
// The census decides this closure; a pass-2 scaffold run on 2026-09-14
// confirmed the candidate is not census refused
// (`docs/package-contract-v2/phase21/2026-09-14-tier-a-pass-2-census.md`).
// What is left is what a mandatory veto is for: call the export and emit only
// on contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. At its call event the export reads `initialValue` off the props object its caller supplied, which is the caller's own (ADR 0034), and builds a signal; building a source is not reading one.
import { default as createControllableSignal } from "@corvu/utils/create/controllableSignal";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const [value, setValue] = createControllableSignal({ initialValue: 3 });
  expect(value() === 3, "the uncontrolled accessor starts at initialValue");
  expect(setValue(5) === 5, "the setter returns the next value");
  expect(value() === 5, "the uncontrolled accessor follows the setter");

  let changed;
  const [controlled, setControlled] = createControllableSignal({
    value: () => 42,
    onChange: (next) => { changed = next; }
  });
  expect(controlled() === 42, "a supplied value accessor takes precedence");
  setControlled(43);
  expect(changed === 43 && controlled() === 42, "a controlled signal reports the change and keeps the caller's value");
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
