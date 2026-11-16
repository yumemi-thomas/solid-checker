import { Show, createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const unused = <Show when={1}>{(value) => <p ref={() => { useContext(Ctx); }}>{value()}</p>}</Show>;
  void unused;
  return <div>x</div>;
}
