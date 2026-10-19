import { createSignal } from "solid-js";
import { render } from "@solidjs/web";
function InBody(props: { label: string }) {
  const [made, setMade] = createSignal("");
  const make = (label: string) => { queueMicrotask(() => setMade(label)); return () => {}; };
  return <div><button onClick={make(props.label)}>x</button><p id="made4">{made()}</p></div>;
}
export function App() {
  const [l] = createSignal("B");
  return <InBody label={l()} />;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
