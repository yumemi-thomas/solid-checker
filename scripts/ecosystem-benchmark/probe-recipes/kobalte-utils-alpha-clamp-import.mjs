// Published JS clamp under the import artifact condition. Finite numeric
// samples and own-global-key observation only; never a completeness proof.
import { clamp } from "@kobalte/utils";

export async function runProbeSession(_session, harness) {
  harness.emit({ marker: "call", kind: "call", phase: "enter" });
  const keys = Reflect.ownKeys(globalThis);
  for (const [value, min, max, expected] of [
    [-1, 0, 10, 0], [0, 0, 10, 0], [5, 0, 10, 5], [11, 0, 10, 10]
  ]) {
    if (clamp(value, min, max) !== expected) throw new Error("clamp numeric result disagrees");
  }
  if (clamp(5) !== 5) throw new Error("clamp defaults disagree");
  if (Reflect.ownKeys(globalThis).some(key => !keys.includes(key))) {
    harness.emit({ marker: "create-operation", kind: "call", phase: "enter" });
  }
  harness.emit({ marker: "call", kind: "call", phase: "exit" });
}
