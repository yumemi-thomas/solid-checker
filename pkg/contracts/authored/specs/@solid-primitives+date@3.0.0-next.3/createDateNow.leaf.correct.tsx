import { onSettled } from "solid-js";
import { createDateNow } from "@solid-primitives/date";
export default function App() {
  createDateNow(0);
  onSettled(() => {});
  return <p>ready</p>;
}
