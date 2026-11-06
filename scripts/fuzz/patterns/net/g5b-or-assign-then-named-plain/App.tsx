import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

function plain() { return 1; }
export function App() {
  let slot: typeof plain | undefined;
  const f1 = () => { (slot ||= plain)(); return 1; };
  const m1 = createMemo(() => new Promise<number>((done) => setTimeout(() => done(1), 1)).then(f1));
  return <Loading fallback="x"><p>{m1()}</p></Loading>;
}
