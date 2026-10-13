/** @jsxImportSource @solidjs/web */
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const visible = createVisibilityObserver(document.body, { initialValue: false });
  return <p>{String(visible())}</p>;
}
