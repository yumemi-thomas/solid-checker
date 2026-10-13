import { onSettled } from "solid-js";
import { createMediaQuery } from "@solid-primitives/media";
export default function App() {
  onSettled(() => { createMediaQuery("(min-width: 1px)"); });
  return <p>candidate</p>;
}
