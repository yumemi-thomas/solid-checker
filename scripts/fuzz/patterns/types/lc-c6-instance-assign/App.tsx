import { createSignal } from "solid-js";
const [w, setW] = createSignal(0);
export function App() {
  const xs = [1, 2];
  Object.assign(xs, { slice: () => { setW(1); return []; } });
  xs.slice(0, 1);
  return <p>{w()}</p>;
}
