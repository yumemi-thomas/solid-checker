import { createMemo, createStore } from "solid-js";
export function StoreFn() {
  const a = createMemo(async () => { await 0; return 1; });
  const [s] = createStore(() => ({ v: a() }), { v: 0 });
  return <time>{s.v}</time>;
}
