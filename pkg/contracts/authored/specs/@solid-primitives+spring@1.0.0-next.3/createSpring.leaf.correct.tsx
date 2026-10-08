/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  createSpring(0);
  onSettled(() => {});
  return <p>ready</p>;
}
