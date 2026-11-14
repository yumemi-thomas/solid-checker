import { createMemo, createRoot, createSignal, createStore } from "solid-js";

const [store] = createStore({ x: 1, n: { x: 1 } });
const [list] = createStore([{ x: 1 }]);
const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B follow-up (ADR 0976), S4 of the review of #233: a store or props value reached through a binding, read after an `await`. `store.n` is a store proxy, so
// `.x` on it is a tracked Get that Chrome rc.21 reports (UNTRACKED_READ_AFTER_AWAIT), and the binding is no symbol of the read universe: a module-scope alias, a destructuring, an element
// of a store array, a function's return, a cell of a literal, an alias made before the `await`, and a props alias. Each function is a declaration handed by identifier to a project wrapper
// (a start the producers' index does not follow), except the alias made before the `await`, which is a recognized computation. `viaPrimitive` is the control: the read happens before the
// `await` and what follows is arithmetic on a number, so nothing is listed after it. Each read is written on one line.
const inner = store.n;
const { n } = store;
const first = list[0];
function pick() {
  return store.n;
}
const got = pick();
const cell: { v: { x: number } } = { v: store.n };
function track(compute: () => Promise<unknown>) {
  return createMemo(compute);
}
async function viaAlias() {
  await pause();
  return inner.x;
}
async function viaDestructured() {
  await pause();
  return n.x;
}
async function viaArray() {
  await pause();
  return first.x;
}
async function viaReturned() {
  await pause();
  return got.x;
}
async function viaCell() {
  await pause();
  return cell.v.x;
}
async function viaPrimitive() {
  const local = store.n.x;
  await pause();
  return local + 1;
}

createRoot(() => {
  track(viaAlias);
  track(viaDestructured);
  track(viaArray);
  track(viaReturned);
  track(viaCell);
  track(viaPrimitive);
  createMemo(async () => {
    const local = store.n;
    await pause();
    return local.x;
  });
});

function Child(props: { n: { y: number } }) {
  const alias = props.n;
  const m = createMemo(async () => {
    await pause();
    return alias.y;
  });
  return <p>{String(m())}</p>;
}
export function Probe() {
  const [t] = createSignal(1);
  return <Child n={{ get y() { return t(); } }} />;
}
