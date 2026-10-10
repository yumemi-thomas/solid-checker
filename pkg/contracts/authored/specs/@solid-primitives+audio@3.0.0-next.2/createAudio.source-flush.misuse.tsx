/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const [url, setUrl] = createSignal("");
  const [sink, setSink] = createSignal(0);
  let calls = 0;
  let armed = false;
  createAudio(() => {
    if (++calls === 1) return untrack(() => url());
    const value = url();
    if (armed) { try { setSink(1); } catch { /* Keep the later compute diagnostic. */ } }
    return value;
  });
  return <><button id="target" onClick={() => {
    armed = true;

    setUrl("#research");
    setTimeout(() => { document.getElementById("done")!.textContent = "done"; }, 30);
  }}>rerun</button><p>{sink()}</p><p id="done">waiting</p></>;
}
