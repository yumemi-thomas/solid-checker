/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  onSettled(() => { try { createTween(() => 1, { duration: 100 }); } catch { /* Preserve structured diagnostic. */ } });
  return <p>ready</p>;
}
