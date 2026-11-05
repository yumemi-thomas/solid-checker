import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush, onCleanup, type Accessor } from "solid-js";
import { render } from "@solidjs/web";
const log = (...a: any[]) => (window as any).__log(...a);
type Fetched<A> = { kind: "loading" } | { kind: "ready"; value: A };
interface Query<A> { state: Accessor<Fetched<A>>; reload: () => void }
function useQuery<A>(o: { key: () => string | undefined }): Query<A> {
  const bound = createMemo(() => o.key(), { lazy: true });
  const state: Accessor<Fetched<A>> = () => { const c = bound(); return c === undefined ? { kind: "loading" } : { kind: "loading" }; };
  return { state, reload: () => { untrack(o.key); } };
}
export function App() { const [id] = createSignal("c"); const q = useQuery<number>({ key: () => id() }); const cur = q.state(); log("k=" + cur.kind); return <div />; }
