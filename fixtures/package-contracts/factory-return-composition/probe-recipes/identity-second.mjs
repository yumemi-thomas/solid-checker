import { value } from "leaf-package";

export async function runProbeSession(_plan, recorder) {
  recorder.emit({ marker: "call", kind: "call", phase: "enter" });
  const second = () => {};
  const result = value({}, second);
  if (!Object.is(result, second)) {
    recorder.emit({ marker: "undeclared-alternative", kind: "callback", phase: "enter" });
  }
  recorder.emit({ marker: "call", kind: "call", phase: "exit" });
}
