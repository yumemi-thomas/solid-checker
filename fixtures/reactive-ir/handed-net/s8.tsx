import { createStore } from "solid-js";
export function App() {
  const [st] = createStore({ l: [{ n: 2 }, { n: 1 }], a: [2, 1] });
  const go = () => {
    Reflect.apply(Array.prototype.sort, st.a, []);
  };
  return <button onClick={go}>{st.l[0].n}</button>;
}
