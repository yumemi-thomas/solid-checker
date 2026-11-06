import { createStore } from "solid-js";
export function App() {
  const [st] = createStore({ a: [2, 1] });
  const more = [3];
  const go = () => { Array.prototype.push.apply(st.a, more); };
  return <button onClick={go}>{st.a[0]}</button>;
}
