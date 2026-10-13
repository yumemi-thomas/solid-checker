import { onSettled } from "solid-js";
import { intersection } from "@solid-primitives/set";
export default function App() {
  intersection(new Set([1]), new Set([2]));
  onSettled(() => {});
  return <p>ready</p>;
}
