import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

export function App() {
  const [r1] = createSignal(3);
  let slot: typeof r1 | undefined;
  const f1 = () => { (slot ||= r1)(); return 1; };
  const m1 = createMemo(() => new Promise<number>((done) => setTimeout(() => done(1), 1)).then(f1));
  return <Loading fallback="x"><p>{m1()}</p></Loading>;
}
