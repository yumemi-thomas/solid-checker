import { createStore } from "solid-js";

type N = { n: number };
const [st] = createStore({
  a: { n: 1 } as N,
  ls: [{ p: { n: 1 } as N }],
  m: { k: { n: 1 } as N } as Record<string, N>,
  o: { v: 1 } as { v?: number },
});
const id = <T,>(x: T): T => x;
const outer = <T,>(y: T): T => id(y);
const deeper = <T,>(z: T): T => outer(z);
const take = ({ a }: { a: N }) => {
  a.n = 7;
};
const gather = (...xs: N[]) => {
  xs[0].n = 8;
};

export function Proved() {
  // Nested identity with the store itself: the call's frame binds `y`. A conditional or a `??` is one of its
  // branches, here the store at its own property.
  return (
    <button
      onClick={() => {
        outer(st).a = { n: 2 };
        ((x: typeof st) => x)(st).a = { n: 3 };
        (Math.random() > 0.5 ? st : st).a = { n: 4 };
        (st ?? st).a = { n: 5 };
      }}
    >
      {st.a.n}
    </button>
  );
}

export function Open() {
  return (
    <button
      onClick={() => {
        for (const { p } of st.ls) p.n = 2;
        const [first] = st.ls;
        first.p.n = 3;
        for (const [, v] of Object.entries(st.m)) v.n = 4;
        (Math.random() > 0.5 ? st.a : { n: 0 }).n = 5;
        [st.a][0].n = 6;
        let late: N;
        late = st.a;
        late.n = 9;
        take(st);
        gather(st.a);
        delete id(st.o).v;
        void (async () => { (await Promise.resolve(st.a)).n = 10; })();
        // Past one frame the walk reads every caller: never proven.
        deeper(st.a).n = 11;
      }}
    >
      {st.a.n}
    </button>
  );
}

export function Silent() {
  return (
    <button
      onClick={() => {
        // A literal written at its own property is fresh, whatever it holds.
        (Math.random() > 0.5 ? { ...st.a } : { n: 0 }).n = 1;
        [...st.ls].reverse();
        // The frame binds the plain argument; the store caller below is not this call.
        outer({ n: 1 }).n = 2;
        void outer(st.a);
        // A conditional's test is not the value.
        (st.a.n > 0 ? { n: 1 } : { n: 2 }).n = 3;
        // A DOM call returns no store proxy.
        (document.getElementById(String(st.a.n)) ?? document.body).dataset.v = "1";
        for (const { p } of [{ p: { n: 1 } }]) p.n = 4;
        take({ a: { n: 1 } });
      }}
    >
      {st.a.n}
    </button>
  );
}
