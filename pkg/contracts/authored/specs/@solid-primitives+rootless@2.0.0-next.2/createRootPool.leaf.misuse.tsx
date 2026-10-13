import { onSettled } from "solid-js";
import { createRootPool } from "@solid-primitives/rootless";
export default function App() {
  onSettled(() => { createRootPool(() => 1); });
  return document.createElement("p");
}
