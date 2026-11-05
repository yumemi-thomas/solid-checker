import { createStore } from "solid-js";

const [m] = createStore<{ a: number | null }>({ a: null });
export const [e] = createStore<{ a: number | null }>({ a: null });
const p: { a?: number } = {};

export function Logical(props: { seed: number }) {
  const [n] = createStore<{ a: number | null }>({ a: null });
  const [z] = createStore({ a: 0, s: "" });
  const [k] = createStore<{ a: number | null }>({ a: 1 });
  const [t] = createStore({ a: 0.0 });
  const [u, setU] = createStore<{ a: number | null }>({ a: null });
  const [v] = createStore<{ a: number | null }>({ a: props.seed });
  setTimeout(() => setU((draft) => { draft.a = 1; }), 0);
  return (
    <>
      {/* Violations: the literal the store still holds passes the test. */}
      <button onClick={() => { n.a ??= 5; }}>nullish</button>
      <button onClick={() => { z.a ||= 5; }}>or</button>
      <button onClick={() => { z.s ||= "x"; }}>or-string</button>
      <button onClick={() => { m.a ??= 5; }}>module</button>
      {/* Silent: the literal fails the test, so nothing is written. */}
      <button onClick={() => { k.a ??= 5; }}>kept</button>
      <button onClick={() => { t.a &&= 5; }}>zero</button>
      {/* Uncertifiable: the value is not proven (a setter, a non-literal, an exported store). */}
      <button onClick={() => { u.a ??= 5; }}>set-later</button>
      <button onClick={() => { v.a ??= 5; }}>seeded</button>
      <button onClick={() => { e.a ??= 5; }}>exported</button>
    </>
  );
}

export function Calls() {
  const [s] = createStore({ a: 1 });
  const [l] = createStore([3, 1, 2]);
  return (
    <>
      {/* Violations: the call always writes. */}
      <button onClick={() => Object.assign(s, { a: 2 })}>assign</button>
      <button onClick={() => l.push(4)}>push</button>
      {/* Uncertifiable: no own key proven, or a length-dependent method. */}
      <button onClick={() => Object.assign(s, {})}>assign-empty</button>
      <button onClick={() => Object.assign(s, p)}>assign-held</button>
      <button onClick={() => l.sort()}>sort</button>
      <button onClick={() => l.reverse()}>reverse</button>
    </>
  );
}
