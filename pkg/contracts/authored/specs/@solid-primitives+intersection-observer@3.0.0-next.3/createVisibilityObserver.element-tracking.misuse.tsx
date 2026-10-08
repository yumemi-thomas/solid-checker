/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [, setSink] = createSignal(0);
  createVisibilityObserver(() => { try { setSink(1); } catch {} return document.body; }, { initialValue: false });
  return <p>ready</p>;
}
