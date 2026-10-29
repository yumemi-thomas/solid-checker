// The control: the same memo reading the signal directly tracks it. Fresh: `#w` shows "1".
import { createMemo, createSignal } from "solid-js";

export default function App() {
  const [count, setCount] = createSignal(0);
  const value = createMemo(() => count());
  return (
    <div>
      <button id="b" onClick={() => setCount(1)}>count</button>
      <span id="w">{value()}</span>
    </div>
  );
}
