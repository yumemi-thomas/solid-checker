// A deliberately vetoed observation: a parent must not acquire receipt
// authority merely because its child's implementation census passed.
export async function runProbeSession(_plan, recorder) {
  recorder.emit({ marker: "call", kind: "call", phase: "enter" });
  recorder.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  recorder.emit({ marker: "call", kind: "call", phase: "exit" });
}
