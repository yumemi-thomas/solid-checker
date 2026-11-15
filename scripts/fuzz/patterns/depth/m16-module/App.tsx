import { createSignal, resolve } from "solid-js";
const [n] = createSignal(1);
const p = () => n();
(() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { (() => { void resolve(p); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })(); })();
export function App() {
  return <i>{String(n())}</i>;
}
