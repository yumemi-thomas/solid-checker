import { createMemo, createSignal, Loading } from "solid-js";
const [count] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
function a() { return count(); }
function b() { return count() + 1; }
function work() { return pause().then(Math.random() > 2 ? b : a); }
export function App() {
  const m = createMemo(work);
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
