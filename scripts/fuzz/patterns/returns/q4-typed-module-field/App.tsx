import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, onCleanup, type Accessor } from "solid-js";
import { render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
type Fetched<A> = { kind: "loading" } | { kind: "ready"; value: A };
interface Query<A> { state: Accessor<Fetched<A>>; reload: () => void }
const reg: { f: Accessor<Fetched<number>> } = { f: () => ({ kind: "loading" }) };
function useQuery(): Query<number> {
  const state: Accessor<Fetched<number>> = () => reg.f();
  return { state, reload: () => {} };
}
export function App() { const [v] = createSignal<Fetched<number>>({ kind: "loading" }); reg.f = v; const q = useQuery(); const cur = q.state(); log("k=" + cur.kind); return <div />; }
