import { createSignal, Show } from "solid-js";
import { render } from "@solidjs/web";
function Header(props: { onOpen?: () => void }) {
  return <div><Show when={props.onOpen}><button onClick={props.onOpen}>open</button></Show></div>;
}
export function App() {
  const [mode, setMode] = createSignal<"a" | "b">("a");
  const [last, setLast] = createSignal("none");
  const handleA = () => setLast("A");
  const handleB = () => setLast("B");
  setTimeout(() => setMode("b"), 0);
  return <div>
    <Header onOpen={mode() === "a" ? handleA : handleB} />
    <p id="last">{last()}</p>
  </div>;
}
export function mount(el: HTMLElement) { return render(() => <App />, el); }
