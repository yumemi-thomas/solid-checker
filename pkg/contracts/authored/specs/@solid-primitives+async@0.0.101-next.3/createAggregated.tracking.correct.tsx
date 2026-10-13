/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createAggregated } from "@solid-primitives/async";
export default function App() {
  const [source] = createSignal(1);
  const [sink, setSink] = createSignal(0);

  const all = createAggregated(() => {
    const value = source();
    /* Only the event handler below writes sink. */
    return value;
  });
  return <><button id="target" onClick={() => {
    setSink(1);
    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p>{String(all())}</p><p id="done">waiting</p></>;
}
