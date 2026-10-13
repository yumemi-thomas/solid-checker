import { value } from "leaf-package";

export async function runProbeSession(_plan, recorder) {
  recorder.emit({ marker: "call", kind: "call", phase: "enter" });
  const input = Object.freeze({ tag: "identity-input" });
  const result = value(input, () => {});
  if (!Object.is(result, input)) {
    recorder.emit({ marker: "undeclared-alternative", kind: "callback", phase: "enter" });
  }
  recorder.emit({ marker: "call", kind: "call", phase: "exit" });
}
