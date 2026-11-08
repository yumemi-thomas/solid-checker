import { createSignal } from "solid-js";
const w: any = document.defaultView;
w.Math = { floor: () => "then" };
const box: any = {};
box[Math.floor(1)] = (run: () => void) => run();
export function App() {
  const [value] = createSignal(1);
  (box as Promise<void>).then(() => { value(); });
  return <p />;
}
