import { createMemo, createSignal, Loading } from "solid-js";
const [count] = createSignal(1);
const pause = () => new Promise<void>((done) => setTimeout(done, 5));
function work() { return new Promise<number>((resolve) => { pause().then(() => 1).then(resolve); }); }
export function App() {
  const m = createMemo(work);
  return <Loading fallback="w"><i>{String(m())}</i></Loading>;
}
