import * as S from "solid-js";
import { createMemo, createRoot, createSignal, getOwner, isPending, latest, runWithOwner, untrack, until } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => {
    void isPending(() => { void until(p); return false; });
    return n();
  });
  return <i>{String(m())}</i>;
}
