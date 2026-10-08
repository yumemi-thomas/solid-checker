/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { mapRange } from "@solid-primitives/range";
export default function App() {
  mapRange(() => 0, () => 3, () => 1, value => value);
  onSettled(() => {});
  return <p>ready</p>;
}
