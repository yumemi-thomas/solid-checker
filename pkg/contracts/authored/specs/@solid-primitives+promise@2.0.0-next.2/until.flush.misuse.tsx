/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { until } from "@solid-primitives/promise";
export default function App() {
  const [epoch, setEpoch] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  let armed = false;
  const pending = until(() => {
    const value = epoch();
    if (armed) { try { setSink(value); } catch { /* Preserve the tracked write diagnostic. */ } }
    return false;
  });
  void pending.catch(() => {});
  return <><button id="target" onClick={() => {
    armed = true;

    setEpoch(1);
    setTimeout(() => { document.getElementById("done")!.textContent = "done"; }, 30);
  }}>rerun</button><p>{sink()}</p><p id="done">waiting</p></>;
}
