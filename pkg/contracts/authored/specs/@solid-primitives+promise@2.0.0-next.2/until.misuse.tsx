/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { until } from "@solid-primitives/promise";
export default function App() {
  const [source] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  const pending = until(() => {
    const value = source();
    try { setSink(1); } catch { /* Keep the write diagnostic without rejecting the probe. */ }
    return value > 0;
  });
  void pending.catch(() => {});
  return <><button id="target" onClick={() => {

    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
