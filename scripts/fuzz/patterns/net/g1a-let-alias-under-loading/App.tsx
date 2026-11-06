import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const a1 = createMemo(async () => 1);
  let useIt = a1;
  return <Loading fallback="x"><p>{useIt()}</p></Loading>;
}
