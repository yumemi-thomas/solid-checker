import { createMemo, createSignal, createContext, useContext, type Accessor } from "solid-js";

type Api = { state: Accessor<number>; go: (n: number) => void };
const C = createContext<Api>({ state: () => 0, go: () => {} });
function Child() {
  const api = useContext(C);
  const v = createMemo(() => {
    const s = api.state();
    api.go(s + 1);
    return s;
  });
  return <p>{v()}</p>;
}
export function App() {
  const [s, setS] = createSignal(0);
  return <C value={{ state: s, go: setS }}><Child /></C>;
}
