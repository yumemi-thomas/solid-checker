import { createMemo, createStore, For, snapshot } from "solid-js";

type N = { n: number };
type R = { id: number; v: number };

// Violations: the call returns the store itself (ADR 0720).
const id = <T,>(v: T): T => v;
const pick = <T,>(_v: unknown, w: T): T => w;
// Uncertifiable: a declaration's identity is not proven exact here.
function same<T>(v: T): T { return v; }

const [st, setSt] = createStore({ n: 1, user: { n: 1 }, items: [{ id: 1, v: 1 }] as R[] });
const whole = createMemo(() => st);
const user = createMemo(() => st.user);
const plainRows: R[] = [{ id: 1, v: 1 }];

export function App() {
  return (
    <>
      <button onClick={() => { id(st).n = 5; }}>identity</button>
      <button onClick={() => { pick(0, st).n = 5; }}>second parameter</button>
      <button onClick={() => { whole().n = 5; }}>memo of the store</button>
      <button onClick={() => { Object.assign(id(st), { n: 5 }); }}>assign through identity</button>
      <button onClick={() => { same(st).n = 5; }}>declaration</button>
      <button onClick={() => { user().n = 5; }}>memo of a link</button>
      <button onClick={() => { st.items.find((x) => x.id === 1)!.v = 5; }}>find</button>
      <button onClick={() => { for (const x of st.items) x.v = 5; }}>for of</button>
      <button onClick={() => { const c = st.items.filter((x) => x.v > 0); c[0].v = 5; }}>filter element</button>
      <button onClick={() => { Object.values(st).forEach((v) => { (v as N).n = 5; }); }}>values</button>
      <For each={st.items}>{(x) => <button onClick={() => { x.v = 2; }}>row</button>}</For>
      <For each={st.items} keyed={false}>{(x) => <button onClick={() => { x().v = 2; }}>unkeyed row</button>}</For>
      <button onClick={() => { st.items.filter((x) => x.v > 0).push({ id: 2, v: 2 }); }}>copy push</button>
      <button onClick={() => { st.items.map((x) => ({ ...x }))[0].v = 5; }}>fresh map</button>
      <button onClick={() => { plainRows.find((x) => x.id === 1)!.v = 5; }}>plain find</button>
      <button onClick={() => { snapshot(st).items.find((x) => x.id === 1)!.v = 5; }}>snapshot find</button>
      <button onClick={() => { id({ n: 1 }).n = 5; }}>plain identity</button>
      <button onClick={() => { id(snapshot(st)).n = 5; }}>snapshot identity</button>
      <button onClick={() => { setSt(() => { id(st).n = 5; }); }}>in setter</button>
      <For each={plainRows}>{(x) => <button onClick={() => { x.v = 2; }}>plain row</button>}</For>
    </>
  );
}
