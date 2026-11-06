import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
const load = (): any => fetch("/x");
export function App() {
  (load() as Promise<Response>).then(() => { setW(1); });
  return <p />;
}
