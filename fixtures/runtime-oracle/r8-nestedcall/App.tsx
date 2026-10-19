import { createSignal } from "solid-js";
import * as Widget from "./widget";
export function C() {
  const [count] = createSignal(0);
  const v = Widget.Root.Child({ read: () => count() });
  return <main>{String(v)}</main>;
}
