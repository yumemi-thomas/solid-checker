import { onSettled } from "solid-js";
import { round } from "@solid-primitives/signal-builders";
export default function App() {
  round(() => 1.2);
  onSettled(() => {});
  return <p>ready</p>;
}
