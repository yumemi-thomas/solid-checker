/** @jsxImportSource @solidjs/web */
import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  const held = createKeyHold("Shift", { preventDefault: false });
  return <p>{String(held())}</p>;
}
