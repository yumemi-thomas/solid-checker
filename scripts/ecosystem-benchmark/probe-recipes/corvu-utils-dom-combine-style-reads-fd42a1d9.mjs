// Hand-authored `reads: []` veto for `@corvu/utils@0.4.2`,
// on the published `./dom` runtime case
// `artifact-case:fd42a1d9dda954dae8c911fad3e828d82b47a4e85f5447e264836f53e6557e5e`
// (33 corpus rows certify through it).
//
// The census decides this closure only since ADR 0107. `combineStyle` writes
// its own `b` parameter, so the `...b` spread roots at ADR 0093's
// `parameter-or-own-result` rather than at `parameter`, and confirming that
// premise means reading `stringStyleToObject`'s transcript -- which the
// `reads` census had no way to ask for until the callee demand landed. What is
// left is what a mandatory veto is for: call the export and emit only on
// contradiction (§ 22 of
// `docs/package-contract-v2/phase21/2026-09-10-reads-veto-observation-design.md`).
//
// NEVER EMITS: `read-operation`. Every property the export reads is an own enumerable property of an object its caller supplied (ADR 0034) or of the plain object `stringStyleToObject` allocates and fills during this same call (ADR 0044). The export owns no reactive source to observe.
import { combineStyle } from "@corvu/utils/dom";
const expect = (ok, what) => { if (!ok) throw new Error(`sample disagrees: ${what}`); };

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });

  expect(
    combineStyle({ color: "red" }, { background: "blue" }).background === "blue",
    "two objects merge"
  );
  expect(
    combineStyle({ color: "red" }, { color: "green" }).color === "green",
    "the second spread wins a collision"
  );

  // The written-parameter arm, which is the whole reason this claim waited on
  // ADR 0107: a string `b` is replaced by the object `stringStyleToObject`
  // allocates, and the spread then reads that object's own properties.
  const parsed = combineStyle({ color: "red" }, "background: blue");
  expect(
    parsed.background === "blue" && parsed.color === "red",
    "a string style is parsed and merged with the object"
  );
  expect(combineStyle({}, "--x: 1")["--x"] === "1", "a custom property survives the parse");
  expect(Object.keys(combineStyle({}, "")).length === 0, "an empty string contributes nothing");

  // The read the spread *does* perform, made observable so that a package edit
  // which stopped reading its caller's object would fail this recipe rather
  // than pass the gate silently. It is the caller's own property (ADR 0034)
  // and so is not this domain's business -- asserting that it fires is what
  // keeps the non-observation above honest.
  let callerReads = 0;
  const supplied = { get color() { callerReads += 1; return "red"; } };
  expect(combineStyle(supplied, {}).color === "red", "the caller's getter value reaches the result");
  expect(callerReads === 1, "the spread read the caller's own accessor exactly once");

  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
