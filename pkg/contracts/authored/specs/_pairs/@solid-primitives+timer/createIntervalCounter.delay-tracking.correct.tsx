/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
export default function App() {
  const [sink, setSink] = createSignal(0);
  untrack(() => createIntervalCounter(() => {
    // The correct callback does not write.
    return 60_000;
  }));
  return <p>{sink()}</p>;
}
