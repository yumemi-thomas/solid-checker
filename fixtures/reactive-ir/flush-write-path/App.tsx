// ADR 0694: in a body only an exact dynamic factory mounts (ADR 0530), a read in
// `flush(fn)` keeps its violation although rc.21 opens no strict window there: a
// violation means wrong if it runs. A store Get that only leads to a write is
// uncertifiable, `write-path-read-unproven`: the same write in the body is, so
// its path claims no more. The write itself stays SC2003.
import { createStore, dynamicComponent, flush } from "solid-js";
import * as Solid from "solid-js";

type State = { a: { v: number; o?: number }; b: { c: { v: number } }; items: number[]; log: { push(n: number): number } };

// Uncertifiable SC1001 at `s.a`, SC2003 at the write.
function PathAssign() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.a.v = 7;
  });
  return <div />;
}

// The whole chain leads to the write.
function PathDeep() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.b.c.v = 7;
  });
  return <div />;
}

// An in-place method answers at its whole callee.
function PathPush() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  Solid.flush(() => {
    s.items.push(1);
  });
  return <div />;
}

function PathDelete() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    delete s.a.o;
  });
  return <div />;
}

function PathStandardWrite() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    Object.assign(s.a, { v: 7 });
  });
  return <div />;
}

// The closure of ADR 0571 runs only in the carved callback.
function PathClosure() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  const wrapUse = () => {
    s.a.v = 7;
  };
  flush(() => {
    wrapUse();
  });
  return <div />;
}

// Violation: a real read.
function RealRead() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    const x = s.a;
    void x;
  });
  return <div />;
}

// Uncertifiable at `s.a`, violation at the value read on the right.
function ValueBeside() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.a.v = s.b.c.v;
  });
  return <div />;
}

// Violation: `+=` reads the value it writes.
function Compound() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.a.v += 1;
  });
  return <div />;
}

// Violation: an alias reads, and a method that writes nothing reads.
function Alias() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    const q = s.a;
    q.v = 7;
  });
  return <div />;
}

function ReadMethod() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    void s.items.map((item) => item);
  });
  return <div />;
}

// Violation: a project method named `push` is no in-place array method.
function Lookalike() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.log.push(1);
  });
  return <div />;
}

// Violation: outside `flush(fn)` of a dynamic-only body this is an ordinary read.
function Ordinary() {
  const [s] = createStore<State>({ a: { v: 0 }, b: { c: { v: 0 } }, items: [0], log: { push: (n: number) => n } });
  flush(() => {
    s.a.v = 7;
  });
  return <div />;
}

export const D1 = dynamicComponent(() => PathAssign);
export const D2 = dynamicComponent(() => PathDeep);
export const D3 = dynamicComponent(() => PathPush);
export const D4 = dynamicComponent(() => PathDelete);
export const D5 = dynamicComponent(() => PathStandardWrite);
export const D6 = dynamicComponent(() => PathClosure);
export const D7 = dynamicComponent(() => RealRead);
export const D8 = dynamicComponent(() => ValueBeside);
export const D9 = dynamicComponent(() => Compound);
export const D10 = dynamicComponent(() => Alias);
export const D11 = dynamicComponent(() => ReadMethod);
export const D12 = dynamicComponent(() => Lookalike);

export function App() {
  return (
    <div>
      <D1 />
      <D2 />
      <D3 />
      <D4 />
      <D5 />
      <D6 />
      <D7 />
      <D8 />
      <D9 />
      <D10 />
      <D11 />
      <D12 />
      <Ordinary />
    </div>
  );
}
