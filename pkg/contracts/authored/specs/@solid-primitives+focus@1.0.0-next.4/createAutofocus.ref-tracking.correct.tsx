/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);

  createAutofocus(() => {
    const value = source();
    /* Only the event handler below writes sink. */
    return null;
  });
  return <><button id="target" onClick={() => {
    setSink(1);
    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
