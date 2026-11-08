import { createSignal, createMemo, Loading } from "solid-js";
const [c] = createSignal(1);
export function App() {
  async function* gen() { yield c(); }
  const m = createMemo(async () => { let last = 0; for await (const v of gen()) last = v; return last; });
  return <Loading fallback="f"><i>{m()}</i></Loading>;
}
