import { createMemo, createSignal, createStore } from "solid-js";
import { isServer } from "@solidjs/web";

const [a, setA] = createSignal(0);
const [st] = createStore({ a: 1 });

// Only `isServer` reaches these: a client build never runs them, proved silent.
function writesStore() { st.a = 2; }
function writesSignal() { setA(1); }
function outer() { inner(); }
function inner() { st.a = 3; }
function ServerOnly() { const v = a(); return <i>{v}</i>; }

// Exported: an importer this project cannot see may run it, so open, not silent.
export function exportedWrite() { st.a = 4; }

// `typeof window` is decided by no selection here: open.
function underConstant() { st.a = 5; }

// A second reference outside any arm: both claims stand.
function mixed() { st.a = 6; }

export function App() {
  if (isServer) {
    writesStore();
    outer();
    exportedWrite();
  }
  if (typeof window === "undefined") underConstant();
  createMemo(() => {
    if (isServer) writesSignal();
    return 1;
  });
  if (isServer) mixed();
  mixed();
  return <main>{isServer ? <ServerOnly /> : null}</main>;
}
