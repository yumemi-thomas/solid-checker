/** @jsxImportSource @solidjs/web */
import { createSelection } from "@solid-primitives/selection";
export default function App() {
  const [selection] = createSelection();
  return <p>{String(selection())}</p>;
}
