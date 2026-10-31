// ADR 0517: an async memo compute named by a named function expression's binding (UNTRACKED_READ_AFTER_AWAIT).
import { createEffect, createMemo, createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const compute = async function named() {
    await Promise.resolve();
    r();
    return 1;
  };
  const m = createMemo(compute);
  createEffect(() => m(), () => {});
  return <span id="w">{r()}</span>;
}
