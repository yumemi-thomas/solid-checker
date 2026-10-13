import { createMemo } from "solid-js";
import { createScheduled, debounce } from "@solid-primitives/scheduled";
export default function App() {
  const scheduled = createScheduled(fn => debounce(fn, 10));
  const current = scheduled();
  void current;
  return document.createElement("p");
}
