import { onSettled } from "solid-js";
import { uppercase } from "@solid-primitives/signal-builders";
export default function App() {
  uppercase(() => "hello");
  onSettled(() => {});
  return <p>ready</p>;
}
