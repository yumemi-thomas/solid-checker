import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  createKeyHold("A");
  return <p>candidate</p>;
}
