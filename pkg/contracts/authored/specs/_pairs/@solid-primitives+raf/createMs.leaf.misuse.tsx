/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createMs } from "@solid-primitives/raf";
export default function App() {
  onSettled(() => { createMs(60); });
  return <p>ready</p>;
}
