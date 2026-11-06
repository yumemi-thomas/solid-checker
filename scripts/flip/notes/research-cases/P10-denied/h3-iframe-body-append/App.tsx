import { createSignal } from "solid-js";
const [c] = createSignal(1);
export function App() {
  const f = <iframe onLoad={() => { c(); }} /> as HTMLIFrameElement;
  document.body.append(f);
  return <p />;
}
