import { createSignal } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
export function App() {
  const location = view(count);
  const copy = Object.assign({}, { location });
  const s = copy.location.search;
  return <i>{s}</i>;
}
