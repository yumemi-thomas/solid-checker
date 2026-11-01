// Legal control: the same closure runs in a click handler.
import { createSignal } from "solid-js";

export default function App() {
  const [r] = createSignal(3);
  const read = () => {
    r.call(null);
  };
  return <button id="w" onClick={() => read()}>{r()}</button>;
}
