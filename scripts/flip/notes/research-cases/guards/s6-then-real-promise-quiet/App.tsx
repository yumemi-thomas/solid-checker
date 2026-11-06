import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  Promise.resolve().then(() => { setW(1); });
  return <p />;
}
