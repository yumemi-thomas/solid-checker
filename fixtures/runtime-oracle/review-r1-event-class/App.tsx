import { onCleanup } from "solid-js";
class Handlers {
  constructor(private cb: () => void) {}
  get onClick() { this.cb(); return () => undefined; }
}
export function App() {
  const h = new Handlers(() => {onCleanup(() => undefined);});
  return <button onClick={h.onClick}>getter</button>;
}
