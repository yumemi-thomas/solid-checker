/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);

  const all = createAggregated(() => {
    const value = source();
    try { setSink(value); } catch { /* Keep the emitted diagnostic and settle the page. */ }
    return value;
  });
  return <><button id="target" onClick={() => {

    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p>{String(all())}</p><p id="done">waiting</p></>;
}
