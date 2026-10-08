/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  const [delay] = createSignal<number | false>(false);
  createTimeoutLoop(() => {}, delay);
  return <p>{String(delay())}</p>;
}
