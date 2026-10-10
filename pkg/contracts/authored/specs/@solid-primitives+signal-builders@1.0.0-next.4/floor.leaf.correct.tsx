import { onSettled } from "solid-js";
import { floor } from "@solid-primitives/signal-builders";
export default function App() {
  floor(() => 1.2);
  onSettled(() => {});
  return <p>ready</p>;
}
