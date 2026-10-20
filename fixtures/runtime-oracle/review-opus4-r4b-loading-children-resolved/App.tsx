import { createMemo, Loading, children } from "solid-js";
import type { JSX } from "@solidjs/web";
const wait = (v: string) => new Promise<string>((r) => setTimeout(() => r(v), 2));
function Layout(props: { children: JSX.Element }) {
  return <main><Loading fallback={<p>loading</p>}>{props.children}</Loading></main>;
}
function Shell(props: { children: JSX.Element }) {
  const c = children(() => props.children);
  return <Layout>{c()}</Layout>;
}
function Page() {
  const data = createMemo(() => wait("f"));
  return <p id="pg3">{data()}</p>;
}
export function App() {
  return <Shell><Page /></Shell>;
}
import { render } from "@solidjs/web";
export function mount(el: HTMLElement) { return render(() => <App />, el); }
