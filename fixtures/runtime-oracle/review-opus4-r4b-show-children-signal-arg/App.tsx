import { createSignal, Show } from "solid-js";
import { render } from "@solidjs/web";
export function App() {
  const [l] = createSignal("S");
  const [open] = createSignal(true);
  const [made, setMade] = createSignal("");
  const make = (label: string) => { queueMicrotask(() => setMade(label)); return () => {}; };
  return <div><Show when={open()}><button onClick={make(l())}>x</button></Show><p id="made2">{made()}</p></div>;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
