import { createSignal } from "solid-js";
import { createHealth, type HealthOptions } from "./health";

// ADR 1056: parameter-default must keep tuple iteration uncertifiable.
createHealth({
  schedule: (delayMs, run = function () { void this; }) => void setTimeout(run, delayMs),
});

export function BodyWrite() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
