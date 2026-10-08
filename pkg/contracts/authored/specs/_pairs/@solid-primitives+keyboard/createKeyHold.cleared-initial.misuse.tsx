/** @jsxImportSource @solidjs/web */
import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  const held = createKeyHold("Shift", { preventDefault: false });
  const current = held();
  return <p>{String(current)}</p>;
}
