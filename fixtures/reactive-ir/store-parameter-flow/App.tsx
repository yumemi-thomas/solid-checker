import { createStore } from "solid-js";

type N = { n: number };

// Violations: a proven store reaches the parameter from a call outside every setter.
const bump = (o: N) => { o.n = 5; };
const each = (f: (o: N) => void) => f(st);
function inner(o: N) { o.n = 1; }
function outer(o: N) { inner(o); }
// Uncertifiable: a link past the root, a caller this build may not see, an element, a later run.
const link = (o: N) => { o.n = 2; };
export function reset(o: N) { o.n = 0; }
const later = (o: N) => () => { o.n = 3; };
// Silent: no store reaches the parameter, or only inside the setter.
const plain = (o: N) => { o.n = 4; };
const drafted = (o: N) => { o.n = 6; };
const inside = (o: N) => { o.n = 7; };
const each2 = (f: (o: N) => void) => f({ n: 0 });
const shadow = (st: N) => { st.n = 8; };

const [st, setSt] = createStore({ n: 1, user: { n: 1 }, items: [{ n: 1 }] });

export function App() {
  return (
    <>
      <button onClick={() => { bump(st as N); }}>root</button>
      <button onClick={() => { each((o) => { o.n = 2; }); }}>callback</button>
      <button onClick={() => { outer(st); }}>two hops</button>
      <button onClick={() => { link(st.user); }}>link</button>
      <button onClick={() => { reset({ n: 1 }); }}>exported</button>
      <button onClick={() => { st.items.forEach((x) => { x.n = 1; }); }}>element</button>
      <button onClick={() => { later(st)(); }}>later</button>
      <button onClick={() => { plain({ n: 0 }); }}>plain</button>
      <button onClick={() => { setSt((d) => { drafted(d); }); }}>draft</button>
      <button onClick={() => { setSt(() => { inside(st); }); }}>in setter</button>
      <button onClick={() => { each2((o) => { o.n = 9; }); }}>plain callback</button>
      <button onClick={() => { shadow({ n: 0 }); }}>shadowed</button>
    </>
  );
}
