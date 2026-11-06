import { createSignal, For } from "solid-js";
const [c] = createSignal(1);
export function App() {
  return <For each={[1]}>{(x) => { const f = <iframe onLoad={() => { c(); }} /> as HTMLIFrameElement; document.body.append(f); return <i />; }}</For>;
}
