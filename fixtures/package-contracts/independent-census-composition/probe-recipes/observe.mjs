import { value } from "root-package";
export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const keys = Reflect.ownKeys(globalThis);
  if (!value(1) || value("1")) throw new Error("typeof control disagrees");
  if (Reflect.ownKeys(globalThis).some(key => !keys.includes(key))) {
    harness.emit({ marker: "undeclared-alternative", kind: "callback", ordinal: 0 });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
