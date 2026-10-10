import { onSettled } from "solid-js";
import { ceil } from "@solid-primitives/signal-builders";
export default function App() {
  ceil(() => 1.2);
  onSettled(() => {});
  return <p>ready</p>;
}
