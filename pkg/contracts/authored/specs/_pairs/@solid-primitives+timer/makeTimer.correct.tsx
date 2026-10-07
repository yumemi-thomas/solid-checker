import { createSignal, untrack } from "solid-js";
import { makeTimer } from "@solid-primitives/timer";
export default function App() {
  const [source] = createSignal(1);
  const provider: typeof setTimeout = (handler, delay, ...args) => {
    untrack(() => source());
    return setTimeout(handler, delay, ...args);
  };
  const stop = makeTimer(() => {}, 1, provider);
  stop();
  return <p>timer provider</p>;
}
