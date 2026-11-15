import { createMemo, createSignal, Loading } from "solid-js";
const [count] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
const handlers = { ok: () => count() };
function work() { return pause().then(handlers.ok); }
export function App() {
  const m = createMemo(work);
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
