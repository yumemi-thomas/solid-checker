import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  const held = createKeyHold("A");
  const value = held();
  return <p>{String(value)}</p>;
}
