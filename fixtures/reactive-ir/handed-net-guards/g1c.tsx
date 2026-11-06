import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const [s] = createSignal(1);
  let useIt = s;
  return <p>{useIt()}</p>;
}
