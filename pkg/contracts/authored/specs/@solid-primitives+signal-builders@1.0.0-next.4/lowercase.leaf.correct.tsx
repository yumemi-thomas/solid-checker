import { onSettled } from "solid-js";
import { lowercase } from "@solid-primitives/signal-builders";
export default function App() {
  lowercase(() => "hello");
  onSettled(() => {});
  return <p>ready</p>;
}
