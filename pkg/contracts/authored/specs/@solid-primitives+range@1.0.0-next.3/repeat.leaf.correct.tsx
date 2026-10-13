/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { repeat } from "@solid-primitives/range";
export default function App() {
  repeat(() => 3, index => index);
  onSettled(() => {});
  return <p>ready</p>;
}
