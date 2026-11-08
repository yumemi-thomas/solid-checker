import { createSignal, createMemo, createEffect, createRoot, getOwner, runWithOwner, onCleanup, Show, For, lazy, mapArray, repeat, reconcile, createStore, untrack, flush, Loading, omit, merge, NotReadyError, resolve, until, children, useContext, createContext, onSettled } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [st, setSt] = createStore({ x: 1, o: { y: 2 }, items: [{ id: 1 }] });
const unk = (): any => null;
const Cx = createContext<{ f: () => number }>({ f: () => 0 });
function Ch() { const x = useContext(Cx);
  const cc = (unk() as (() => number) | null) ?? x.f;
  createEffect(() => c2(), () => { log("apply", own()); const v = cc(); log("apply", "read " + String(v)); });
  return <i />;
}
export function App() {

  return <Cx value={{ f: c }}><Ch /></Cx>;
}
