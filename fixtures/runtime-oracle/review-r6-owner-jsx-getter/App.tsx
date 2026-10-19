import { createRoot, onCleanup } from "solid-js";
import { render } from "@solidjs/web";
function Holder(props: { value: void }) {
  createRoot(() => { void props.value; });
  return <span>held</span>;
}
function handlers(cb: () => void) {
  return { onClick: (event: MouseEvent) => {
    const element = <Holder value={cb()} />;
    const host = document.createElement("section");
    (event.currentTarget as HTMLElement).after(host);
    render(() => element, host);
  }};
}
export function App() {
  const h = handlers(() => onCleanup(() => undefined));
  return <button {...h}>click</button>;
}
