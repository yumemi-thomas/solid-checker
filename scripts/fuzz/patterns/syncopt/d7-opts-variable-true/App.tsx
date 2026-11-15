import { createMemo, createSignal, Loading } from "solid-js";
const [n] = createSignal(1);
const over: { sync?: boolean } = { sync: false };
const opts = { sync: true };
export function App() {
  try {
    const m = createMemo(async () => 1, opts);
    return <Loading fallback="..."><i>{String(m())}</i></Loading>;
  } catch (e) {
    return <i>threw</i>;
  }
}
