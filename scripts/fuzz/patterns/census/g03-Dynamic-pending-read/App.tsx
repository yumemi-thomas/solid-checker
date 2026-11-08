import { createSignal, createMemo, createEffect, createRoot, getOwner, onCleanup, Show, For, lazy, dynamicComponent, mapArray, reconcile, createStore, untrack, NotReadyError, Loading } from "solid-js";
import { Dynamic, clientOnly, dynamic } from "@solidjs/web";
const log = (...a: any[]) => { (window as any).__log?.(...a); };
const own = () => (getOwner() ? "owner" : "no-owner");
const [c, setC] = createSignal(1);
const [c2, setC2] = createSignal(1);
const [w, setW] = createSignal(0);
const [on, setOn] = createSignal(true);
let runs = 0;
function HiddenP(props: { a: () => number | undefined }) { log("HiddenP", "body " + own()); try { const v = props.a(); log("HiddenP", "read " + v); } catch (e) { log("HiddenP", "threw " + (e as Error).name); } return <i>h</i>; }
export function App() {
  const a = createMemo(async () => { await new Promise<void>(() => {}); return 1; });
  return <Loading fallback="f"><Dynamic component={HiddenP} a={a} /></Loading>;
}
