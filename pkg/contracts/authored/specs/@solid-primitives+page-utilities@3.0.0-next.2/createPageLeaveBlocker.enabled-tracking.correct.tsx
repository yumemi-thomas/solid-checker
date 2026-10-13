import { createSignal } from "solid-js";
import { createPageLeaveBlocker } from "@solid-primitives/page-utilities";
export default function App() {
  const [enabled] = createSignal(true); const [sink, setSink] = createSignal(0);
  createPageLeaveBlocker(() => { const value = enabled();  return value; });
  return <p>{sink()}</p>;
}
