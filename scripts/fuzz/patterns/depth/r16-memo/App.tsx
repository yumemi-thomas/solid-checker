import { createMemo, createSignal, resolve } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
export function App() {
  const m = createMemo(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { void resolve(p); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); return n(); });
  return <i>{String(m())}</i>;
}
