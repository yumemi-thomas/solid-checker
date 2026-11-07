import { createMemo, createSignal } from "solid-js";

document.getElementById("ready")!.title = "settled";

export function App() {
  const [s, set] = createSignal(0);
  const m = createMemo(() => {
    [1].forEach(() => { set(1); });
    return document.getElementById("x") ? s() : 0;
  });
  return <p>{m()}</p>;
}
