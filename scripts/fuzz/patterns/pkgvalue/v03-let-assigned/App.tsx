import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
export function App() {
  let location;
  location = view(count);
  const s = location.search;
  return <i>{s}</i>;
}
