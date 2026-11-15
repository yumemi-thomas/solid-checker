import { createContext, createSignal, useContext } from "solid-js";
import { view } from "fz-view";
const [count] = createSignal(1);
const Ctx = createContext<{ readonly search: number }>();
function Consumer() {
  const v = useContext(Ctx);
  const s = v.search;
  return <i>{s}</i>;
}
export function App() {
  const v = view(count);
  return <Ctx value={v}><Consumer /></Ctx>;
}
