import { onSettled } from "solid-js";
import { createReducedMotion } from "@solid-primitives/a11y";
export default function App() {
  createReducedMotion();
  onSettled(() => {});
  return <p>ready</p>;
}
