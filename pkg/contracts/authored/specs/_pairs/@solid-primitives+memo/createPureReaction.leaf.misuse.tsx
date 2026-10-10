/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createPureReaction } from "@solid-primitives/memo";
export default function App() {
  onSettled(() => { createPureReaction(() => {}); });
  return <p>ready</p>;
}
