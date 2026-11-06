import { createSignal, onCleanup } from "solid-js";
import { render } from "@solidjs/web";
const [c] = createSignal(1);
function Inner() { onCleanup(() => { c(); }); return <i />; }
export function App() {
  const host = document.createElement("div");
  const dispose = render(() => <Inner />, host);
  dispose();
  return <p />;
}
