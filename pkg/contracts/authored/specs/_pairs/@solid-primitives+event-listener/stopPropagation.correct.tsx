import { createEffect, createMemo, createSignal } from "solid-js";
import { stopPropagation } from "@solid-primitives/event-listener";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);
  const handler = stopPropagation(() => {
    const value = source();
    // The wrapper preserves caller tracking; imperative writes belong in apply.
  });
  createMemo(() => {
    handler(new Event("click", { cancelable: true }));
    return 0;
  });
  createEffect(() => source(), value => { setSink(value); });
  return <p>{sink()}</p>;
}
