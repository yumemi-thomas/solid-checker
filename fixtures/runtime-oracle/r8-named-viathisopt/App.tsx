import { createSignal } from "solid-js";
import { viaThisOpt, Root } from "./widget";
export function C() {
  const [count] = createSignal(0);
  const v = viaThisOpt(() => count());
  return <Root>{v}</Root>;
}
