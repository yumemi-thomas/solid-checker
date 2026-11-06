import { createSignal, createMemo, Loading } from "solid-js";
const [c] = createSignal(1);
export function App() {
  async function* gen() { await Promise.resolve(); yield c(); }
  const m = createMemo(async () => { const it = gen(); const r = await it.next(); return r.value as number; });
  return <Loading fallback="f"><i>{m()}</i></Loading>;
}
