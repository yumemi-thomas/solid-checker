import { createSignal } from "solid-js";
import * as Widget from "./widget";
export function C() {
  const [count] = createSignal(0);
  return <main><Widget.Root.Child read={() => count()} /></main>;
}
