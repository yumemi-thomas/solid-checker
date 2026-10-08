/** @jsxImportSource @solidjs/web */
import { createMutationObserver } from "@solid-primitives/mutation-observer";
export default function App() {
  createMutationObserver(() => document.body, { childList: true }, () => {});
  return <p>ready</p>;
}
