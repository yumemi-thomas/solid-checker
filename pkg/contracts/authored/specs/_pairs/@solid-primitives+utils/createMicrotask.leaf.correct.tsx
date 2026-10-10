import { onSettled } from "solid-js";
import { createMicrotask } from "@solid-primitives/utils";
export default function App() {
  createMicrotask(() => {});
  return <p>ready</p>;
}
