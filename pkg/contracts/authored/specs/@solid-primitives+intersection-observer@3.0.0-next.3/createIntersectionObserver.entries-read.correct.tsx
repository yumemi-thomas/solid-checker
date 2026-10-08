/** @jsxImportSource @solidjs/web */
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [entries] = createIntersectionObserver(() => [document.body]);
  return <p>{String(entries.length)}</p>;
}
