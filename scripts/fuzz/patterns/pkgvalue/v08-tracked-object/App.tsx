import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
export function App() {
  const o = { location: view(count) };
  return <i>{o.location.search}</i>;
}
