import { createSignal, untrack } from "solid-js";
import { makeEventListener } from "@solid-primitives/event-listener";
// OBSERVATION: returned callable is only a bound platform removal, no state machine.
export default function App() {
  const [done, setDone] = createSignal("");
  const target = new EventTarget();
  let calls = 0;
  const remove = makeEventListener(target, "batch1-remove", () => { calls++; });
  const run = () => setTimeout(() => {
    target.dispatchEvent(new Event("batch1-remove"));
    remove();
    target.dispatchEvent(new Event("batch1-remove"));
    setDone(calls === 1 ? "done" : "unexpected");
  }, 0);
  return <><button id="target" onClick={run}>remove</button><p id="done">{done()}</p></>;
}
