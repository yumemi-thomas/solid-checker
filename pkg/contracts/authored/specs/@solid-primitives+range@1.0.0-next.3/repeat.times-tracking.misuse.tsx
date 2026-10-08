/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { repeat } from "@solid-primitives/range";
export default function App() {
  const [times] = createSignal(3);
  const [, setSink] = createSignal(0);
  repeat(() => { const n = times(); try { setSink(1); } catch {} return n; }, index => index);
  return <p>ready</p>;
}
