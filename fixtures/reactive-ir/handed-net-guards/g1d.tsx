import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const [s] = createSignal(1);
  const box = { useIt: s };
  return <p>{box.useIt()}</p>;
}
