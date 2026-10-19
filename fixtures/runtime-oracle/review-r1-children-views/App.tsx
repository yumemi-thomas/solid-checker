import { createSignal, children, merge, omit, createMemo } from "solid-js";
import type { JSX } from "@solidjs/web";
function Destructured({children: child}: {children: () => JSX.Element}) { return <div>{child()}</div>; }
function Merged(props: {children: () => JSX.Element}) {const p = merge(props); return <div>{p.children()}</div>;}
function Memo(props: {children: () => JSX.Element}) {const m = createMemo(() => props.children()); return <div>{m()}</div>;}
export function App() {
  const [n] = createSignal(0);
  return <><Destructured>{() => {const v=n(); return <span>{v}</span>;}}</Destructured><Merged>{() => {const v=n(); return <span>{v}</span>;}}</Merged><Memo>{() => {const v=n(); return <span>{v}</span>;}}</Memo></>;
}
