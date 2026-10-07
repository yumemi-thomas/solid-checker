import { createSignal } from "solid-js";
import { access } from "@solid-primitives/utils";
export default function App() {
  const [source] = createSignal(3);
  const seed = access(source);
  return <p>{seed}</p>;
}

