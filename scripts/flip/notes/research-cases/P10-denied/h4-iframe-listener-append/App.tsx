import { createSignal } from "solid-js";
const [c] = createSignal(1);
export function App() {
  const f = document.createElement("iframe");
  f.addEventListener("load", () => { c(); });
  document.body.append(f);
  return <p />;
}
