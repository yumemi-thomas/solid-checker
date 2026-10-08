/** @jsxImportSource @solidjs/web */
import { onSettled, untrack } from "solid-js";
import { createIntervalCounter } from "@solid-primitives/timer";
export default function App() {
  untrack(() => createIntervalCounter(60_000));
  onSettled(() => {});
  return <p>ready</p>;
}
