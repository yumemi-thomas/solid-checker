import { createEffect, createMemo, createSignal } from "solid-js";
import { createMemoCache } from "@solid-primitives/memo";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const cached = createMemoCache((key: number) => {
    const value = source();
    try { setSink(value); } catch { /* Diagnostic emitted; keep the probe mounted. */ }
    return key + value;
  });
  createMemo(() => cached(1));
  
  return <p>{sink()}</p>;
}
