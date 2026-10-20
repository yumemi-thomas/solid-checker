import { createMemo, Loading } from "solid-js";
import type { JSX } from "@solidjs/web";
const wait = (v: string) => new Promise<string>((r) => setTimeout(() => r(v), 2));
function Layout(props: { children: JSX.Element }) {
  return <main><Loading fallback={<p>loading</p>}>{props.children}</Loading></main>;
}
function Shell(props: { children: JSX.Element }) {
  const content = () => props.children;
  return <Layout>{content()}</Layout>;
}
function Page() {
  const data = createMemo(() => wait("d"));
  return <p id="pg">{data()}</p>;
}
export function App() {
  return <Shell><Page /></Shell>;
}
import { render } from "@solidjs/web";
export function mount(el: HTMLElement) { return render(() => <App />, el); }
