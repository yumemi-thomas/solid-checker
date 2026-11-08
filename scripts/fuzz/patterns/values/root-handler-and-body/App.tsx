import { createSignal, createMemo, createEffect, createRoot, runWithOwner, getOwner, createStore, untrack, flush, onCleanup, mapArray, repeat, Show, For, Switch, Match, reconcile, Loading, createContext, useContext, type Accessor } from "solid-js";
import { Dynamic, render } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const [c] = createSignal(1);
export function App() { const h = () => { log('ran'); createRoot(() => c()); }; h(); return <button onClick={h} />; }
