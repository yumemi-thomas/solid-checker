import { createSignal } from "solid-js";

// health.ts has script syntax: this proof has no strict-module evidence for the helper.
createHealth({ schedule: (delayMs, run) => void setTimeout(run, delayMs) });
export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
