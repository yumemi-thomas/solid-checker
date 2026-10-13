import { onSettled } from "solid-js";
import { createPageVisibility } from "@solid-primitives/page-utilities";
export default function App() {
  createPageVisibility();
  onSettled(() => {});
  return <p>ready</p>;
}
