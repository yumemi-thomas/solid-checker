/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);

  createAutofocus(() => {
    const value = source();
    try { setSink(value); } catch { /* Keep the emitted diagnostic and settle the page. */ }
    return null;
  });
  return <><button id="target" onClick={() => {

    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
