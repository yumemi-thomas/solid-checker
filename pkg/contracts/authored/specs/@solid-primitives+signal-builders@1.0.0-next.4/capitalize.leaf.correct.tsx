import { onSettled } from "solid-js";
import { capitalize } from "@solid-primitives/signal-builders";
export default function App() {
  capitalize(() => "hello");
  onSettled(() => {});
  return <p>ready</p>;
}
