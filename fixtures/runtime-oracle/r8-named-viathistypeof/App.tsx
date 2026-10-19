import { createSignal } from "solid-js";
import { viaThisTypeof, Root } from "./widget";
export function C() {
  const [count] = createSignal(0);
  const v = viaThisTypeof(() => count());
  return <Root>{v}</Root>;
}
