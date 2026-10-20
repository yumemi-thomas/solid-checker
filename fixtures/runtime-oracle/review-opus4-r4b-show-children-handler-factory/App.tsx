import { createSignal, Show } from "solid-js";
import { render } from "@solidjs/web";
function InShow(props: { open: boolean; label: string }) {
  const [made, setMade] = createSignal("");
  const make = (label: string) => { queueMicrotask(() => setMade(label)); return () => {}; };
  return <div><Show when={props.open}><button onClick={make(props.label)}>x</button></Show><p id="made">{made()}</p></div>;
}
export function App() {
  const [l] = createSignal("L");
  const [o] = createSignal(true);
  return <InShow open={o()} label={l()} />;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
