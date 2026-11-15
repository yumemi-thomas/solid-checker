import * as S from "solid-js";
import { createMemo, createRoot, createSignal, getOwner, isPending, latest, runWithOwner, untrack, until } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => {
    class K { run() { return until(p); } } void new K().run();
    return n();
  });
  return <i>{String(m())}</i>;
}
