import { onSettled } from "solid-js";
import { createActiveElement } from "@solid-primitives/active-element";
export default function App() {
  createActiveElement();
  onSettled(() => {});
  return <p>ready</p>;
}
