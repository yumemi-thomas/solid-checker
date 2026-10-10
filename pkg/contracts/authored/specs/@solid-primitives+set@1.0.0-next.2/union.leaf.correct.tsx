import { onSettled } from "solid-js";
import { union } from "@solid-primitives/set";
export default function App() {
  union(new Set([1]), new Set([2]));
  onSettled(() => {});
  return <p>ready</p>;
}
