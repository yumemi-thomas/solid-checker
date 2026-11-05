import { createSignal, createMemo, createStore, untrack, merge, omit, snapshot, createContext, useContext, createEffect, flush, For } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [st, setSt] = createStore({ a: 1 });
  return <div><button onClick={() => { try { Object.freeze(st); log("freeze ok"); } catch (e: any) { log("freeze threw " + e.message); } try { st.a = 5; log("write ok"); } catch (e: any) { log("write threw " + e.message); } }}>go</button></div>;
}
