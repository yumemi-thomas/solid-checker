/** @jsxImportSource @solidjs/web */
import { createSignal, getObserver } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  const [source, setSource] = createSignal(0);
  const [sink, setSink] = createSignal(0);
  createTimeoutLoop(() => {}, () => {
    const value = source();
    if (getObserver() && true) { try { setSink(1); } catch { /* Keep owned-write diagnostic. */ } }
    return false;
  });
  return <><button id="target" onClick={() => {


    setTimeout(() => { document.getElementById("done")!.textContent = "done"; }, 20);
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
