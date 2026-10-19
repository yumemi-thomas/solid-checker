import { createSignal } from "solid-js";
import * as Widget from "./widget";
import later = Widget.readLater;
export function C() {
  const [count] = createSignal(0);
  later(() => count());
  return <Widget.Root>x</Widget.Root>;
}
