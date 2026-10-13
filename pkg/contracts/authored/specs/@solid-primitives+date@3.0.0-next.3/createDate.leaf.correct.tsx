import { onSettled } from "solid-js";
import { createDate } from "@solid-primitives/date";
export default function App() {
  createDate(0);
  onSettled(() => {});
  return <p>ready</p>;
}
