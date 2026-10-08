import { onSettled } from "solid-js";
import { difference } from "@solid-primitives/set";
export default function App() {
  difference(new Set([1]), new Set([2]));
  onSettled(() => {});
  return <p>ready</p>;
}
