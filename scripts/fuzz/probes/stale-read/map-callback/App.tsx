// ADR 0443 (1): the read runs untracked in mapArray's map function, so the row never updates (rc.17 is silent). Stale: `#w` keeps "1a" (fresh would be "1z").
import { createSignal, mapArray } from "solid-js";

export default function App() {
  const [list] = createSignal([1]);
  const [tag, setTag] = createSignal("a");
  const rows = mapArray(list, (item) => `${item}${tag()}`);
  return (
    <div>
      <button id="b" onClick={() => setTag("z")}>tag</button>
      <span id="w">{rows()[0]}</span>
    </div>
  );
}
