import "./runtime";
import { createSignal } from "solid-js";
import * as Widget from "./widget";
export function C() {
  const [count] = createSignal(0);
  Widget.readLater(() => count());
  return <Widget.Root>x</Widget.Root>;
}
