import { createEffect, createMemo, createSignal } from "solid-js";
import { stopPropagation } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const handler = stopPropagation(() => {
    const value = source();
    try { setSink(value); } catch { /* Diagnostic remains attributed to this setter. */ }
  });
  createMemo(() => {
    handler(new Event("click", { cancelable: true }));
    return 0;
  });

  return <p>{sink()}</p>;
}
