import { createMemo, createStore, Loading } from "solid-js";
const [store] = createStore({ n: { x: 1 } });
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
export function App() {
  const m = createMemo(async () => { const local = store.n; await pause(); local.x++; return 1; });
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
