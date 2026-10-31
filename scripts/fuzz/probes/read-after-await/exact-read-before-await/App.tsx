// Legal control: the named compute reads before its first await.
import { createEffect, createMemo, createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const compute = async function named() {
    r();
    await Promise.resolve();
    return 1;
  };
  const m = createMemo(compute);
  createEffect(() => m(), () => {});
  return <span id="w">{r()}</span>;
}
