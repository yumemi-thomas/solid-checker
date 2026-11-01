// ADR 0517: an async memo compute named by an object literal's member (UNTRACKED_READ_AFTER_AWAIT).
import { createEffect, createMemo, createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const box = { compute: async () => {
    await Promise.resolve();
    r();
    return 1;
  } };
  const m = createMemo(box.compute);
  createEffect(() => m(), () => {});
  return <span id="w">{r()}</span>;
}
