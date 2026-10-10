/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  const [fps] = createSignal(60);
  const [, setSink] = createSignal(0);
  createMs(() => { const value = fps(); try { setSink(1); } catch {} return value; });
  return <p>ready</p>;
}
