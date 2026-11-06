import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  setTimeout(() => { setW(1); }, 0);
  return <p />;
}
