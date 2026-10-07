import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  const held = createKeyHold("A");
  return <p>{String(held())}</p>;
}
