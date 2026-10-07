import { createShortcut } from "@solid-primitives/keyboard";
export default function App() {
  createShortcut(["A"], () => {});
  return <p>candidate</p>;
}
