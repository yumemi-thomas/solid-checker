import "./runtime";
import { createSignal } from "solid-js";
import * as Widget from "./widget";
declare function sink(x: unknown): void;
export function C() {
  const [count] = createSignal(0);
  Widget.readLater(() => count());
  sink({ ...Widget });
  return <Widget.Root>x</Widget.Root>;
}
