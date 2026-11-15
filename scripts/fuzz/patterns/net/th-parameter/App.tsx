import { createMemo, createSignal, Loading } from "solid-js";
const [count] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
function chain(handler: () => number) { return pause().then(handler); }
function work() { return chain(() => count()); }
export function App() {
  const m = createMemo(work);
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
