/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const [url] = createSignal("");
  const [sink, setSink] = createSignal(0);
  let calls = 0;
  createAudio(() => {
    if (++calls === 1) return untrack(() => url());
    const value = url();
    /* Only the event handler writes sink. */
    return value;
  });
  return <><button id="target" onClick={() => {
    setSink(1);
    document.getElementById("done")!.textContent = "done";
  }}>finish</button><p>{sink()}</p><p id="done">waiting</p></>;
}
