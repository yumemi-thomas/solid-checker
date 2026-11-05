import { createStore } from "solid-js";
export function App() { const [s] = createStore({ a: 0 }); const h = () => { if (Math.random() > 2) s.a = 1; }; h(); return <i />; }
