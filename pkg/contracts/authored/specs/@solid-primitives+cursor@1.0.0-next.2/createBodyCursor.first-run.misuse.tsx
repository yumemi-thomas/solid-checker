import { createSignal } from "solid-js";
import { createBodyCursor } from "@solid-primitives/cursor";
export default function App() {
  const [source] = createSignal("pointer" as const);
  const [sink, setSink] = createSignal(0);
  const [done, setDone] = createSignal("");
  let runs = 0;
  createBodyCursor(() => {
    runs++;
    const value = source();
    try { setSink(1); } catch { /* Expected owned-write rejection. */ }
    return value;
  });
  if (runs < 1) throw new Error("mandatory first compute did not run");
  const finish = () => { setDone("done"); };
  return <><button id="target" onClick={finish}>finish</button><p>{sink()}</p><p id="done">{done()}</p></>;
}
