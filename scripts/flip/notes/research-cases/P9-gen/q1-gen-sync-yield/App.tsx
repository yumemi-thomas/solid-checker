import { createSignal, createMemo, Loading } from "solid-js";
const [c] = createSignal(1);
export function App() {
  function* gen() { yield c(); }
  const m = createMemo(async () => { await 0; let last = 0; for (const v of gen()) last = v; return last; });
  return <Loading fallback="f"><i>{m()}</i></Loading>;
}
