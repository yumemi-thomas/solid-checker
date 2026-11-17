import { createSignal } from "solid-js";
import { createHealth, type HealthOptions } from "./health";
let kept: HealthOptions | undefined;

// ADR 1056: member-receiver must keep tuple iteration uncertifiable.
createHealth({
  schedule: function (delayMs, run) { kept = this; void setTimeout(run, delayMs); },
});

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
