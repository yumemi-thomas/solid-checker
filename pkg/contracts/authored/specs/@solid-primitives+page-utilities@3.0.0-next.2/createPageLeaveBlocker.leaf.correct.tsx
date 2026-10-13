import { onSettled } from "solid-js";
import { createPageLeaveBlocker } from "@solid-primitives/page-utilities";
export default function App() {
  createPageLeaveBlocker(true);
  onSettled(() => {});
  return <p>ready</p>;
}
