/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createClipboard } from "@solid-primitives/clipboard";
export default function App() {
  const [source, setSource] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  createClipboard(() => {
    const value = source();

    return String(value);
  });
  return <><button id="target" onClick={() => {
    setSource(1);
    setSink(1);
    setTimeout(() => { document.getElementById("done")!.textContent = "done"; }, 20);
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
