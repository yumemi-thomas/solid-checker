/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  createVisibilityObserver(() => document.body, { initialValue: false });
  return <p>ready</p>;
}
