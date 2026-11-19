import { createMemo, createSignal } from "solid-js";
import "fz-global-route-pattern-source";
function Wrap() {
  const [ready, setReady] = createSignal(false);
  setTimeout(() => setReady(true), 100);
  const decoded = createMemo(() => (ready() ? atob("eA==") : ""));
  return <p>{decoded()}</p>;
}
export function App() {
  return <Wrap />;
}
