import { createEffect, createMemo, createSignal } from "solid-js";
import { preventDefault } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const handler = preventDefault(() => {
    const value = source();
    try { setSink(value); } catch { /* Diagnostic remains attributed to this setter. */ }
  });
  createMemo(() => {
    handler(new Event("click", { cancelable: true }));
    return 0;
  });

  return <p>{sink()}</p>;
}
