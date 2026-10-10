import { onSettled } from "solid-js";
import { createMarker } from "@solid-primitives/marker";
export default function App() {
  createMarker(() => document.createElement("mark"));
  return document.createElement("p");
}
