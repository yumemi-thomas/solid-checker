/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  createTween(() => 1, { duration: 100 });
  onSettled(() => {});
  return <p>ready</p>;
}
