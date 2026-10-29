// ADR 0484: flush() in a closure called in place in a createEffect apply drains nothing (FLUSH_IN_EFFECT_CALLBACK in dev).
import { createEffect, createSignal, flush } from "solid-js";

export default function App() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    () => {
      const reveal = () => flush();
      reveal();
    },
  );
  return <span id="w">{count()}</span>;
}
