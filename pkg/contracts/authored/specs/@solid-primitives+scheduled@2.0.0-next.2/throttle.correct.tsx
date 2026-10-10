import { onSettled } from "solid-js";
import { throttle } from "@solid-primitives/scheduled";
export default function App() {
  throttle(() => {}, 10);
  return document.createElement("p");
}
