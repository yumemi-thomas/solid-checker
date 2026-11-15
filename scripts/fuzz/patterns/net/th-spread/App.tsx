import { createMemo, createSignal, Loading } from "solid-js";
const [count] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
function work() { const fns = [() => count()] as const; return pause().then(...fns); }
export function App() {
  const m = createMemo(work);
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
