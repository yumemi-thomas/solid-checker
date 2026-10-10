import { onSettled } from "solid-js";
import { symmetricDifference } from "@solid-primitives/set";
export default function App() {
  symmetricDifference(new Set([1]), new Set([2]));
  onSettled(() => {});
  return <p>ready</p>;
}
