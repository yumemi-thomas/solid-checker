/** @jsxImportSource @solidjs/web */
import { createRoot } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";

// Built outside any component, so only the read below is under test.
const count = createRoot(() => createIntervalCounter(1000));

export default function App() {
  return <p>{String(count())}</p>;
}
