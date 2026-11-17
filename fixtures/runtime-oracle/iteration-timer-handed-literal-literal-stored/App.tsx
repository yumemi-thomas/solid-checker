import { createSignal } from "solid-js";
import { createHealth, type HealthOptions } from "./health";
let kept: HealthOptions;
createHealth((kept = {
  schedule: (delayMs, run) => void setTimeout(run, delayMs),
}));
kept.schedule(1000, function () { void this; });

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
