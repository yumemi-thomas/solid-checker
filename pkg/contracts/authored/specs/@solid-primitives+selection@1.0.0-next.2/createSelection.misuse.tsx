/** @jsxImportSource @solidjs/web */
import { createSelection } from "@solid-primitives/selection";
export default function App() {
  const [selection] = createSelection();
  const frozen = selection();
  return <p>{String(frozen)}</p>;
}
