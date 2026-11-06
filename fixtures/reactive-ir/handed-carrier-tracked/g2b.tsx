import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const a1 = createMemo(async () => 1);
  const hf = () => { a1(); return 1; };
  return <p>{([] as number[]).map(hf)}</p>;
}
