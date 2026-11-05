import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, onCleanup, type Accessor } from "solid-js";
import { render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
type Fetched<A> = { kind: "loading" } | { kind: "ready"; value: A };
interface Query<A> { state: Accessor<Fetched<A>>; reload: () => void }
function useQuery<A>(value: Accessor<Fetched<A>>): Query<A> {
  const state: Accessor<Fetched<A>> = () => value();
  return { state, reload: () => {} };
}
export function App() { const [v] = createSignal<Fetched<number>>({ kind: "loading" }); const q = useQuery<number>(v); const cur = q.state(); log("k=" + cur.kind); return <div />; }
