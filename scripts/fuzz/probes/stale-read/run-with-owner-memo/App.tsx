// ADR 0443 (3): runWithOwner clears tracking, so the memo's read in its callback subscribes nothing and the memo never recomputes
// (rc.17 is silent). Stale: `#w` keeps "0" (fresh would be "1").
import { createMemo, createSignal, getOwner, runWithOwner } from "solid-js";

export default function App() {
  const owner = getOwner();
  const [count, setCount] = createSignal(0);
  const value = createMemo(() => runWithOwner(owner, () => count()));
  return (
    <div>
      <button id="b" onClick={() => setCount(1)}>count</button>
      <span id="w">{value()}</span>
    </div>
  );
}
