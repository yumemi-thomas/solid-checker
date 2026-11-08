import { createMemo, createSignal } from "solid-js";
import { render } from "@solidjs/web";

const host = () => document.body.appendChild(document.createElement("div"));
const wait = () => new Promise<number>((done) => setTimeout(() => done(1), 5));

// `render` calls it as `code()`: a root, no strict window. Proved silent.
function Quiet() {
  const [a] = createSignal(0);
  const v = a();
  return <p>{v}</p>;
}
render(Quiet, host());

// The same root: a pending read throws, and the write keeps its claim.
function Pending() {
  const m = createMemo(async () => await wait());
  const v = m();
  return <p>{v}</p>;
}
render(Pending, host());

function Writes() {
  const [a, setA] = createSignal(0);
  setA(1);
  return <p>{a()}</p>;
}
render(Writes, host());

// A child goes through `createComponent`: its window is back.
function Child() {
  const [a] = createSignal(0);
  const v = a();
  return <p>{v}</p>;
}
function Parent() {
  return <Child />;
}
render(Parent, host());

// Mounted both ways: the `<Both />` has the window, so the read warns there.
function Both() {
  const [a] = createSignal(0);
  const v = a();
  return <p>{v}</p>;
}
render(Both, host());
export const App = () => <Both />;

// An alias is not an exact mount.
function Aliased() {
  const [a] = createSignal(0);
  const v = a();
  return <p>{v}</p>;
}
const Alias = Aliased;
render(Alias, host());
