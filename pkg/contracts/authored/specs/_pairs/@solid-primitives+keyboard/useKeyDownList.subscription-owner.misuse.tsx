import { onSettled } from "solid-js";
import { useKeyDownList } from "@solid-primitives/keyboard";
export default function App() {
  onSettled(() => { useKeyDownList(); });
  return <p>candidate</p>;
}
