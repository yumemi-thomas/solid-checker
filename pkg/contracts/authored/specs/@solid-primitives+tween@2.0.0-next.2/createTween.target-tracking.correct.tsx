/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  createTween(() => 1, { duration: 100 });
  return <p>ready</p>;
}
