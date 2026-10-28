// The control: the input commits every edit to its signal, so the rewrite writes what the user typed (no warning).
import { createSignal } from "solid-js";

export default function App() {
  const [title, setTitle] = createSignal("start");
  const [tone, setTone] = createSignal(0);
  return (
    <div>
      <input id="i" value={title()} onInput={(event) => setTitle(event.currentTarget.value)} class={tone() ? "on" : "off"} />
      <button id="b" onClick={() => setTone((n) => n + 1)}>tone</button>
      <span id="w">{tone() > 0 ? 1 : 0}</span>
    </div>
  );
}
