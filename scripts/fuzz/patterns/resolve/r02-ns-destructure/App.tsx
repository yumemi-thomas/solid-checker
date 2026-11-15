import * as S from "solid-js";
import { createMemo, createRoot, createSignal, getOwner, isPending, latest, runWithOwner, untrack, resolve } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => {
    const { resolve: q } = S; void q(p);
    return n();
  });
  return <i>{String(m())}</i>;
}
