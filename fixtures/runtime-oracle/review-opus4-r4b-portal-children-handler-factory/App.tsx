import { createSignal } from "solid-js";
import { render, Portal } from "@solidjs/web";
function InPortal(props: { label: string }) {
  const [made, setMade] = createSignal("");
  const make = (label: string) => { queueMicrotask(() => setMade(label)); return () => {}; };
  return <div><Portal><button onClick={make(props.label)}>x</button></Portal><p id="made3">{made()}</p></div>;
}
export function App() {
  const [l] = createSignal("P");
  return <InPortal label={l()} />;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
