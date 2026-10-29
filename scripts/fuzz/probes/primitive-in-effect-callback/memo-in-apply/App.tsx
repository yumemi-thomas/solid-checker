// ADR 0483: a memo created in a createEffect apply with no owner throws PRIMITIVE_IN_EFFECT_CALLBACK (rc.17 dev).
import { createEffect, createMemo, createSignal } from "solid-js";

export default function App() {
  const [count] = createSignal(0);
  createEffect(
    () => count(),
    (value) => {
      createMemo(() => value * 2);
    },
  );
  return <span id="w">{count()}</span>;
}
