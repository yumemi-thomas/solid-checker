import { createSignal } from "solid-js";
import { viaThis } from "./widget";
export function C() {
  const [count] = createSignal(0);
  const v = viaThis(() => count());
  return <main>{v}</main>;
}
