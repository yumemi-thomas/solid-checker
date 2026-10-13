/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { until } from "@solid-primitives/promise";
export default function App() {
  const [source] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  const pending = until(() => {
    const value = source();
    /* A read-only condition is valid. */
    return value > 0;
  });
  void pending.catch(() => {});
  return <><button id="target" onClick={() => {
    setSink(1);
    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
