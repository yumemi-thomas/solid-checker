// The control: the bound value itself changes, so the write is the program's new value, not a rewrite of the old one (no warning).
import { createSignal } from "solid-js";

export default function App() {
  const [title, setTitle] = createSignal("start");
  return (
    <div>
      <input id="i" value={title()} />
      <button id="b" onClick={() => setTitle((t) => `${t}!`)}>next</button>
      <span id="w">{title() === "start" ? 0 : 1}</span>
    </div>
  );
}
