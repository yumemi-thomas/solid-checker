/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createSwitchTransition } from "@solid-primitives/transition-group";
export default function App() {
  createSwitchTransition(() => 1, {});
  onSettled(() => {});
  return <p>ready</p>;
}
