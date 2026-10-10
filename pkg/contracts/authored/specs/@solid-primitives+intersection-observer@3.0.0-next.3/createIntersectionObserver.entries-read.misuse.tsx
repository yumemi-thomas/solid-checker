/** @jsxImportSource @solidjs/web */
import { createIntersectionObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const [entries] = createIntersectionObserver(() => [document.body]);
  const frozen = entries.length;
  return <p>{String(frozen)}</p>;
}
