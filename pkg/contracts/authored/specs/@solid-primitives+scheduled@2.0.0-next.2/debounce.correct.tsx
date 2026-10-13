import { onSettled } from "solid-js";
import { debounce } from "@solid-primitives/scheduled";
export default function App() {
  debounce(() => {}, 10);
  return document.createElement("p");
}
