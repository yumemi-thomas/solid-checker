import { createSignal } from "solid-js";
import { callbacks } from "./source";
export function Callback() {
  const [localCount] = createSignal(0);
  callbacks.invoke(() => { void localCount(); });
  return <p>no callback runs</p>;
}
