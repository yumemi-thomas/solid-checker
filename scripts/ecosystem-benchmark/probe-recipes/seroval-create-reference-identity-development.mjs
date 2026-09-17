import { createReference } from "seroval";

// `createReference`'s `returns` closure: the census proved the export returns
// parameter 1 by identity (`output: {index: 1, kind: "parameter", path: []}`),
// and this recipe is the mandatory contradiction veto that proof schedules.
//
// The published body is `REFERENCE.set(value, id); INV_REFERENCE.set(id, value);
// return value` — two plain `Map`s, so unlike a `WeakMap`-backed registry it
// accepts primitives as keys and the sample can cross the primitive/object
// boundary rather than stopping at objects.
//
// Only a contradiction is claimed. Finite samples cannot establish the
// closure; the authenticated implementation census does that (ADR 0008), and
// this run only has to fail to falsify it.
export async function runProbeSession(_session, harness) {
  const object = { tag: "solid-checker:reference-probe" };
  const samples = [
    ["solid-checker:object", object],
    ["solid-checker:frozen", Object.freeze({ tag: "frozen" })],
    ["solid-checker:function", function sample() {}],
    ["solid-checker:string", "value"],
    ["solid-checker:zero", 0],
    ["solid-checker:negative-zero", -0],
    ["solid-checker:nan", Number.NaN],
    ["solid-checker:null", null],
    ["solid-checker:undefined", undefined],
    ["solid-checker:symbol", Symbol("sample")]
  ];
  for (const [id, input] of samples) {
    harness.emit({ marker: "call", kind: "call", phase: "enter" });
    const output = createReference(id, input);
    if (!Object.is(output, input)) {
      harness.emit({ marker: "return-outside-identity", kind: "call", phase: "enter" });
    }
    harness.emit({ marker: "call", kind: "call", phase: "exit" });
  }
  // The same value under a second id still returns that value: re-registration
  // is the one path where a registry-backed factory could plausibly hand back
  // a previously stored object instead of its argument.
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  if (!Object.is(createReference("solid-checker:object-again", object), object)) {
    harness.emit({ marker: "return-outside-identity", kind: "call", phase: "enter" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
