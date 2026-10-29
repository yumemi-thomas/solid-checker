// (A counter: the oracle clicks every element before the steps.) The input's value is bound to a signal nothing writes; a sibling attribute of the same element changes after the user typed.
// Solid's grouped render effect rewrites `value` with the unchanged value over the typed text (UNCOMMITTED_INPUT_OVERWRITTEN).
import { createSignal } from "solid-js";

export default function App() {
  const [title] = createSignal("start");
  const [tone, setTone] = createSignal(0);
  return (
    <div>
      <input id="i" value={title()} class={tone() ? "on" : "off"} />
      <button id="b" onClick={() => setTone((n) => n + 1)}>tone</button>
      <span id="w">{tone() > 0 ? 1 : 0}</span>
    </div>
  );
}
