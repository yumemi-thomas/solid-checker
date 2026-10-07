import { createEffect, createMemo, createSignal } from "solid-js";
import { createMemoCache } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const cached = createMemoCache((key: number) => {
    const value = source();
    // Pure tracked calculation; the write is in effect apply below.
    return key + value;
  });
  createMemo(() => cached(1));
  createEffect(() => source(), value => { setSink(value); });
  return <p>{sink()}</p>;
}
