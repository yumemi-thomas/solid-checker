import { onSettled } from "solid-js";
import { scheduleIdle } from "@solid-primitives/scheduled";
export default function App() {
  scheduleIdle(() => {}, 10);
  return document.createElement("p");
}
