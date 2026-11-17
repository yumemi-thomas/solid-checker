import { createSignal } from "solid-js";
import { createHealth, type HealthOptions } from "./health";
let saved: HealthOptions["schedule"] | undefined;
createHealth({
  schedule: (saved = (delayMs, run) => void setTimeout(run, delayMs)),
});
saved?.(1000, function () { void this; });

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
