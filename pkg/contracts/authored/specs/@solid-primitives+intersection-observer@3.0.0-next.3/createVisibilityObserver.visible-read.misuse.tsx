/** @jsxImportSource @solidjs/web */
import { createVisibilityObserver } from "@solid-primitives/intersection-observer";
export default function App() {
  const visible = createVisibilityObserver(document.body, { initialValue: false });
  const frozen = visible();
  return <p>{String(frozen)}</p>;
}
