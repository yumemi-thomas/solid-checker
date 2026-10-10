/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const [pattern] = createSignal(200);
  createVibrate(() => pattern());
  return <p>ready</p>;
}
