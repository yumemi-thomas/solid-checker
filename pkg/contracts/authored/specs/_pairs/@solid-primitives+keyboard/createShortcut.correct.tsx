import { createShortcut } from "@solid-primitives/keyboard";
export default function App() {
  createShortcut(["Control", "K"], () => {});
  return <p>candidate</p>;
}
