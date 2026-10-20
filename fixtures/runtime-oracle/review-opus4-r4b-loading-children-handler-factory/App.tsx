import { createSignal, Show, Loading, Errored } from "solid-js";
import { render } from "@solidjs/web";
import type { JSX } from "@solidjs/web";
const [made, setMade] = createSignal("");
const make = (id: number) => { queueMicrotask(() => setMade("m" + id)); return () => {}; };
const select = (id: number) => setMade("s" + id);
function X(props: { id: number; open: boolean }) {
  return <Loading fallback="..."><button onClick={make(props.id)}>c</button></Loading>;
}
export function App() {
  const [id] = createSignal(1);
  const [o] = createSignal(true);
  return <div><X id={id()} open={o()} /><p id="made">{made()}</p></div>;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
