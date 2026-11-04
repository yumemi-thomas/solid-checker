import { createSignal, createStore, untrack } from "solid-js";

declare function register(fn: () => void): void;

// A read in an intrinsic `ref` the body applies: the same role however it reads.
export function RefRead() {
  const [s] = createStore({ a: { v: 0 } });
  return <p ref={() => {
    const x = s.a; // SC1001
  }}>x</p>;
}

export function RefWrite() {
  const [s] = createStore({ a: { v: 0 } });
  return <p ref={() => {
    s.a.v = 7; // SC1001 at the link, SC2003 at the write
  }}>x</p>;
}

export function RefSpread() {
  const [s] = createStore({ a: 0 });
  return <p ref={() => {
    const x = { ...s }; // SC1001
  }}>x</p>;
}

function Kid(props: { v: number }) {
  return <p ref={() => {
    const x = props.v; // SC1001: the parent passes a read
  }}>x</p>;
}

export function RefProp() {
  const [g] = createSignal(1);
  return <Kid v={g()} />;
}

export function RefUntracked() {
  const [s] = createStore({ a: { v: 0 } });
  return <p ref={() => {
    untrack(() => s.a); // clean
  }}>x</p>;
}

// A named function a carrier runs: the read is placed at the carrier.
export function Handed() {
  const [s] = createStore({ a: { v: 0 } });
  const hf = () => {
    const x = s.a; // SC1001
  };
  [1].forEach(hf);
  return <p>x</p>;
}

export function HandedWrite() {
  const [s] = createStore({ a: { v: 0 } });
  const hf = () => {
    s.a.v = 7; // SC1001 at the link, SC2003 at the write
  };
  [1].forEach(hf);
  return <p>x</p>;
}

export function FreshStack() {
  const [s] = createStore({ a: { v: 0 } });
  const hf = () => {
    const x = s.a; // clean: a timer's task
  };
  setTimeout(hf, 0);
  return <p>x</p>;
}

export function Unfollowed() {
  const [s] = createStore({ a: { v: 0 } });
  const hf = () => {
    const x = s.a; // uncertifiable: nothing says when `register` runs it
  };
  register(hf);
  return <p>x</p>;
}

export function Shadowed() {
  const [s] = createStore({ a: { v: 0 } });
  const hf = (s: { a: number }) => {
    const x = s.a; // clean: the parameter, not the store
  };
  [{ a: 1 }].forEach(hf);
  return <p>x</p>;
}

// A `delete` reads nothing, however its function is reached.
export function DeleteThroughClosure() {
  const [s] = createStore({ b: 1 } as { b?: number });
  const wrap = () => {
    delete s.b; // no SC1001
  };
  const f = () => {
    wrap();
  };
  f();
  return <p>x</p>;
}
