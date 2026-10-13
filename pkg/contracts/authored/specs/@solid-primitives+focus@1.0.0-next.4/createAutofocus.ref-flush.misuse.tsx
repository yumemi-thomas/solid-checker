/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAutofocus } from "@solid-primitives/focus";
export default function App() {
  const [epoch, setEpoch] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  let armed = false;
  createAutofocus(() => {
    const value = epoch();
    if (armed) { try { setSink(value); } catch { /* Keep the callback diagnostic. */ } }
    return null;
  });
  return <><button id="target" onClick={() => {
    armed = true;

    setEpoch(1);
    setTimeout(() => { document.getElementById("done")!.textContent = "done"; }, 30);
  }}>rerun</button><p>{sink()}</p><p id="done">waiting</p></>;
}
