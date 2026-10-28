// The control: JSX the map function returns tracks its own expressions, so the row updates. Fresh: `#w` shows "1z".
import { createSignal, mapArray } from "solid-js";

export default function App() {
  const [list] = createSignal([1]);
  const [tag, setTag] = createSignal("a");
  const rows = mapArray(list, (item) => <i>{item}{tag()}</i>);
  return (
    <div>
      <button id="b" onClick={() => setTag("z")}>tag</button>
      <span id="w">{rows()}</span>
    </div>
  );
}
